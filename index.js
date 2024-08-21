import init, { Test } from "./core.js";

// Initialization
(async () => { await init(); exec(); })();

function exec() {
    const body = document.querySelector("body");

    document.querySelector("button").addEventListener("click", () => {
        const element = () => {
            const li = document.createElement("div");
            li.textContent = Test.hello();
            return li;
        };

        body.appendChild(element());
    });
}