const zjb = new Zjb();

const loadExports = async (name, initial) => {
    const params = {
        zjb: zjb.imports,
        env: {
            memory: new WebAssembly.Memory({ initial: initial }),
            __stack_pointer: 0,
        },
    };

    const request = fetch(name);
    const module = await WebAssembly.instantiateStreaming(request, params);
    zjb.setInstance(module.instance);

    return zjb.exports;
};

const init = async _ => {
    const exports = await loadExports('core.wasm', 10);

    const features = document.getElementById('features');

    features.onwheel = event => {
        event.preventDefault();
        features.scrollLeft += event.deltaY;
    };

    for (const featurePath of exports.getFeatures()) {
        let temp = document.createElement('button');

        temp.appendChild(document.createTextNode(featurePath));
        temp.onclick = _ => console.log(featurePath);

        features.appendChild(temp);
    }

    const canvas = document.querySelector('body > canvas');
    
    const resizeCanvas = _ => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
    };

    resizeCanvas();

    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);
    
        sinceLastResize = setTimeout(resizeCanvas, 250);
    };
};

init();

