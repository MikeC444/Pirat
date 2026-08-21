use pirat_launcher_lib::db::{self, downloads_repo, games_repo, installs_repo, sources_repo};
use pirat_launcher_lib::install;
use pirat_launcher_lib::models::NewGameSource;
use pirat_launcher_lib::sources;
use std::sync::Mutex;

/// End-to-end smoke test wiring together everything a real user session
/// would touch: add the bundled example catalogue as a source, sync it,
/// search/filter/sort the resulting library, download a game from a local
/// HTTP server, and run it through the install pipeline (checksum + extract).
#[tokio::test]
async fn full_catalogue_to_install_flow() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let catalogue_path = repo_root.join("example-data/games.json");
    assert!(catalogue_path.exists(), "example-data/games.json must exist");

    let conn = Mutex::new(db::open_in_memory().unwrap());
    let client = reqwest::Client::new();

    // 1. Add the bundled example catalogue as a source and sync it.
    let source_id = {
        let guard = conn.lock().unwrap();
        sources_repo::create(
            &guard,
            &NewGameSource {
                name: "Example Catalogue".into(),
                url: format!("file://{}", catalogue_path.display()),
                update_interval_minutes: 60,
            },
        )
        .unwrap()
    };
    let source = { sources_repo::get(&conn.lock().unwrap(), source_id).unwrap().unwrap() };
    let summary = sources::sync_source(&conn, &client, &source).await.unwrap();
    assert_eq!(summary.games_upserted, 7, "expected all 7 example games to parse");
    assert!(summary.warnings.is_empty(), "example catalogue should be well-formed: {:?}", summary.warnings);

    // 2. Search/filter/sort the resulting library through the same query path
    // the UI uses.
    let all_games = { games_repo::query(&conn.lock().unwrap(), &Default::default()).unwrap() };
    assert_eq!(all_games.len(), 7);

    let searched = {
        games_repo::query(
            &conn.lock().unwrap(),
            &games_repo::GameQuery {
                search: Some("verdant".into()),
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].title, "Verdant Hollow");

    let racing = {
        games_repo::query(
            &conn.lock().unwrap(),
            &games_repo::GameQuery {
                genre: Some("Racing".into()),
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert_eq!(racing.len(), 1);
    assert_eq!(racing[0].title, "Gridlock Racer X");

    // 3. Simulate re-syncing while the source is temporarily unreachable: the
    // previously cached catalogue must remain intact (offline resilience).
    let broken_source = pirat_launcher_lib::models::GameSource {
        url: "file:///this/path/does/not/exist.json".into(),
        ..source.clone()
    };
    let err = sources::sync_source(&conn, &client, &broken_source).await.unwrap_err();
    assert!(err.contains("failed to read local catalogue file"));
    let still_cached = { games_repo::query(&conn.lock().unwrap(), &Default::default()).unwrap() };
    assert_eq!(still_cached.len(), 7, "catalogue must stay available when the source is down");

    // 4. Download a game from a local HTTP server standing in for a real
    // download host, using the same DownloadProvider the app uses.
    let game = searched[0].clone(); // "Verdant Hollow"
    let body = b"pretend game installer payload bytes".to_vec();
    let (server_url, _handle) = serve_once(body.clone()).await;

    let dest_dir = std::env::temp_dir().join(format!("pirat-e2e-{}", uuid::Uuid::new_v4()));
    // Not a .zip on purpose: the install pipeline places non-archive files
    // as-is, which this test exercises alongside the zip-extraction path
    // already covered by install::tests::install_extracts_zip_and_finds_executable.
    let dest_path = dest_dir.join("verdant-hollow.bin");

    let download_id = {
        let guard = conn.lock().unwrap();
        downloads_repo::create(&guard, game.id, dest_path.to_string_lossy().as_ref(), "t0").unwrap()
    };

    use pirat_launcher_lib::downloads::provider::{DownloadControl, DownloadProvider, DownloadRequest};
    use pirat_launcher_lib::downloads::http_provider::HttpDownloadProvider;
    use std::sync::atomic::AtomicI64;
    use std::sync::Arc;

    let provider = HttpDownloadProvider::new(client.clone());
    let control = Arc::new(DownloadControl::new(Arc::new(AtomicI64::new(0))));
    let outcome = provider
        .download(
            DownloadRequest {
                uri: server_url,
                dest_path: dest_path.clone(),
            },
            control,
            Box::new(|_, _| {}),
        )
        .await
        .unwrap();
    assert_eq!(
        outcome,
        pirat_launcher_lib::downloads::provider::DownloadOutcome::Completed
    );
    {
        let guard = conn.lock().unwrap();
        downloads_repo::set_state(&guard, download_id, "completed", "t1").unwrap();
    }
    assert_eq!(std::fs::read(&dest_path).unwrap(), body);

    // 5. Run the downloaded file through the install pipeline (no checksum on
    // this game, so it should just extract/place successfully) and record the
    // installation.
    let install_root = std::env::temp_dir().join(format!("pirat-e2e-install-{}", uuid::Uuid::new_v4()));
    let install_result = install::install_downloaded_file(&dest_path, &install_root, &game.title, None, None).unwrap();
    {
        let guard = conn.lock().unwrap();
        installs_repo::upsert(
            &guard,
            game.id,
            &install_result.install_path.to_string_lossy(),
            None,
            game.version.as_deref(),
            "t2",
        )
        .unwrap();
    }
    let install_record = { installs_repo::get_by_game(&conn.lock().unwrap(), game.id).unwrap().unwrap() };
    assert_eq!(install_record.install_path, install_result.install_path.to_string_lossy());

    // Cleanup.
    let _ = std::fs::remove_dir_all(&dest_dir);
    let _ = std::fs::remove_dir_all(&install_root);
}

async fn serve_once(body: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.write_all(&body).await;
        let _ = stream.shutdown().await;
    });
    (format!("http://{addr}"), handle)
}
