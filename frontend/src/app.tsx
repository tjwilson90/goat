import React from "react";
import { render } from "react-dom";

/** Main entry point to the game. */
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