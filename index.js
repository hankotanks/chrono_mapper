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

        const scaleCanvas = _ => {
            const canvas = document.getElementsByTagName("canvas").item(0);

            if(canvas != undefined) {
                let x = canvas.width / window.innerWidth;
                let y = canvas.height / window.innerHeight;

                if(x > y) {
                    canvas.style.width = `${window.innerWidth}px`;
                    canvas.style.height = `${canvas.height / x}px`;
                } else {
                    canvas.style.width = `${canvas.width / y}px`;
                    canvas.style.height = `${window.innerHeight}px`;
                }
            }
        };

        setTimeout(scaleCanvas, 1);
    }; resizeCanvas();

    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);
    
        sinceLastResize = setTimeout(resizeCanvas, 300);
    };
};

(async () => { await init(); exec(); })();
