import init, { Wrapper } from "./core.js";

// Initialization
const exec = async () => {
    Wrapper.run();

    const resizeCanvas = _ => {
        Wrapper.update_canvas(String(window.innerWidth), String(window.innerHeight));
    };
    
    resizeCanvas();
    
    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);
    
        sinceLastResize = setTimeout(resizeCanvas, 300);
    };
};

(async () => { await init(); exec(); })();