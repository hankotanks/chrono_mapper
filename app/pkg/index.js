import init, { Wrapper } from "./core.js";

const exec = async () => {
    Wrapper.run();

    window.onload = _ => {
        const canvas = document.getElementsByTagName("canvas").item(0);
        canvas.focus();
        canvas.onblur = _ => { setTimeout(_ => { canvas.focus(); }, 1); };
    };
};

(async () => { await init(); exec(); })();
