import React from "react";
import { render } from "react-dom";

export const App = () => {
    return (
        <div>
            <h1>React App!</h1>
        </div>
    );
};

// Mount react on startup.
const root = document.getElementById("root");
render(<App />, root);