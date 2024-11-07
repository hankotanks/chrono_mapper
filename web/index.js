// instantiate the zig-js bridge
const zjb = new Zjb();

// helper function to load a WASM module with `initial` pages of memory
const loadModule = async (name, initial) => {
    // configure WASM memory
    const params = {
        zjb: zjb.imports,
        env: {
            memory: new WebAssembly.Memory({ initial: initial }),
            __stack_pointer: 0,
        },
    };
    // request the module
    const request = fetch(name);
    // init WASM
    const module = await WebAssembly.instantiateStreaming(request, params);
    // configure ZJB
    zjb.setInstance(module.instance);
    // return memory buffer and ZJB exports
    return {
        memory: module.instance.exports.memory,
        exports: zjb.exports,
    };
};

const parseFeatures = data => {
    // object MUST have a type AND type MUST be a FeatureCollection
    if (!('type' in data)) throw new Error('Failed to parse feature layer');
    const type = data['type'];
    if (type != 'FeatureCollection') throw new Error('Failed to parse feature layer');
    // feature collecton must have an array of constituent features
    if (!('features' in data)) throw new Error('Failed to parse feature layer');
    const features = [];
    for (const entry of data['features']) {
        // feature MUST have a properties field
        if (!('properties' in entry)) continue;
        const properties = entry['properties'];
        // AND it MUST contain a non-null name field
        if (!('NAME' in properties)) continue;
        const name = properties['NAME'];
        if (name == null) continue;
        // feature MUST have geometry with a set of coordinates
        if (!('geometry' in entry)) continue;
        const geometry = entry['geometry'];
        if (!('type' in geometry)) continue;
        if (geometry['type'] != 'MultiPolygon') continue;
        if (!('coordinates' in geometry)) continue;
        const coordinates = geometry['coordinates'];
        if (coordinates.length == 0) continue;
        // restructure feature
        const temp = {
            "name": name,
            "coordinates": coordinates,
        };
        // add to layer
        features.push(temp);
    }
    // return null if the feature layer is empty, otherwise return it
    if (features.length == 0) return null;
    return features;
};

const init = async _ => {
    // instantiate WASM module
    const { exports, memory } = await loadModule('core.wasm', 10);

    // get handle to canvas and its context
    const canvas = document.querySelector('body > canvas');
    const canvasContext = canvas.getContext('2d');

    // helper function to send a feature layer to zig
    const renderLayer = layer => {
        // encode as a Uint8Array
        const layerArr = (new TextEncoder()).encode(JSON.stringify(layer));
        // request WASM memory to store the Uint8Array
        const layerByteOffset = exports.allocArrayBuffer(layerArr.length, 1);
        // create a window of sufficient size into WASM memory
        const layerView = new Uint8Array(memory.buffer, layerByteOffset, layerArr.length);
        // write feature layer to WASM memory
        layerView.set(layerArr);
        // render the feature layer
        return exports.renderLayer(
            canvasContext,
            layerByteOffset,
            layerArr.length,
            canvas.width, 
            canvas.height,
        );
    };

    // configure scrolling feature layer selector
    const featureList = document.getElementById('features');
    featureList.onwheel = event => {
        featureList.scrollLeft += event.deltaY;
        event.preventDefault();
    };

    // populate feature layer selector
    var selectedFeature;
    for (const featurePath of exports.getLayers()) {
        // create a button for the current feature path
        let temp = document.createElement('button');
        // append it to the DOM
        featureList.appendChild(temp);
        // add label
        temp.appendChild(document.createTextNode(featurePath));
        // handle render behavior
        temp.onclick = _ => {
            const url = `${location.href}${featurePath}`;
            fetch(url).then(response => {
                if (response.ok) { return response.json(); }
                throw new Error(`Request failed [${response.status}].`);
            }).then(parseFeatures).then(features => {
                // clear canvas if there are valid features to render
                if (!features.length) return;
                canvasContext.clearRect(0, 0, canvas.width, canvas.height);
                // render features
                if (renderLayer({ "name": featurePath, "features": features })) {
                    throw new Error('Unable to render feature layer.');
                }
                // mark feature layer as selected
                temp.style.background = 'darkseagreen';
                // unselect previous feature layer
                if (selectedFeature && !selectedFeature.isEqualNode(temp)) {
                    selectedFeature.style.background = 'transparent';
                }
                // store current selection
                selectedFeature = temp;
            }).catch(error => {
                temp.style.backgroundColor = 'lightpink';
                console.error(`${featurePath}:\n${error}`);
            });
        };
    }
    
    // helper function to resize canvas
    const resizeCanvas = _ => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
        if (selectedFeature) {
            selectedFeature.click();
        }
    };

    // resize once on load
    resizeCanvas();

    // resize canvas whenever the viewport dimensions change
    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);
        sinceLastResize = setTimeout(resizeCanvas, 250);
    };
};

init();

