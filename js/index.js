import init, { Wrapper } from "./core.js";

const exec = async () => {
    Wrapper.run();

    let sinceLastFocus;
    const focusCanvas = _ => {
        clearTimeout(sinceLastFocus);

        const canvas = document.getElementsByTagName("canvas").item(0);

        if(canvas == undefined) {
            sinceLastFocus = setTimeout(focusCanvas, 300);
        } else {
            canvas.focus();
            
            canvas.onblur = _ => {
                setTimeout(_ => { canvas.focus(); }, 1);
            };
        }
    }; focusCanvas();

    const resizeCanvas = _ => {
        Wrapper.set_screen_resolution(
            String(window.innerWidth), 
            String(window.innerHeight)
        );
    }; resizeCanvas();

    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);
    
        sinceLastResize = setTimeout(resizeCanvas, 300);
    };
};

(async () => { await init(); exec(); })();
