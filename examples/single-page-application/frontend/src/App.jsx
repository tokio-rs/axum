import * as React from "react";
import { Outlet, Link } from "react-router-dom";

export default function App() {
  return (
    <div>
      <h1>axum single page application example</h1>
        <nav
            style={{
                borderBottom: "solid 1px",
                paddingBottom: "1rem",
            }}
        >
            <Link to="/users">Users</Link> |{" "}
            <Link to="/blogs">Blogs</Link> |{" "}
            <Link to="/about">About</Link>
        </nav>
        <Outlet />
    </div>
  );
}
