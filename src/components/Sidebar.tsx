import { NavLink } from "react-router-dom";

const LINKS = [
  { to: "/", label: "Home", icon: "⌂" },
  { to: "/library", label: "Library", icon: "▦" },
  { to: "/downloads", label: "Downloads", icon: "⤓" },
  { to: "/sources", label: "Sources", icon: "↻" },
  { to: "/settings", label: "Settings", icon: "⚙" },
];

export function Sidebar() {
  return (
    <nav className="sidebar">
      <div className="sidebar-brand">
        <span className="sidebar-brand-mark">P</span>
        Pirat Launcher
      </div>
      <div className="sidebar-nav">
        {LINKS.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.to === "/"}
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
          >
            <span className="sidebar-link-icon">{link.icon}</span>
            {link.label}
          </NavLink>
        ))}
      </div>
      <div className="sidebar-footer">Pirat Launcher v0.1.0</div>
    </nav>
  );
}
