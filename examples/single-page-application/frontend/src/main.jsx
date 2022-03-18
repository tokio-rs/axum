import * as React from "react";
import * as ReactDOM from "react-dom";
import {
    BrowserRouter,
    Routes,
    Route,
} from "react-router-dom";
import App from "./App";
import Users from "./routes/users";
import Blogs from "./routes/blogs";
import About from "./routes/about";
import "./main.css";

let rootElement = document.getElementById("root");
ReactDOM.render(
    <BrowserRouter>
        <Routes>
            <Route path="/" element={<App />}>
                <Route path="users" element={<Users />} />
                <Route path="blogs" element={<Blogs />} />
                <Route path="about" element={<About />} />
                <Route
                    path="*"
                    element={
                        <main style={{ padding: "1rem" }}>
                            <p>Oops! There's nothing here!</p>
                        </main>
                    }
                />
            </Route>
        </Routes>
    </BrowserRouter>,
    rootElement
);
