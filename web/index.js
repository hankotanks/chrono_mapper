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
            "geometry": [],
        };
        // populate parsed feature
        for (const ring of coordinates) {
            temp.geometry.push(ring.flat(2));
        }
        // add to layer
        features.push(temp);
    }
    // return null if the feature layer is empty, otherwise return it
    if (features.length == 0) return null;
    return features;
};

const init = async _ => {
    const { exports, memory } = await loadExports('core.wasm', 10);

    const canvas = document.querySelector('body > canvas');

    const featureList = document.getElementById('features');
    featureList.onwheel = event => {
        featureList.scrollLeft += event.deltaY;
        event.preventDefault();
    };

    var selectedFeature;
    for (const featurePath of exports.getFeatures()) {
        let temp = document.createElement('button');
        featureList.appendChild(temp);

        temp.appendChild(document.createTextNode(featurePath));
        temp.onclick = _ => {
            const url = `${location.href}${featurePath}`;
            fetch(url).then(response => {
                if (response.ok) { return response.json(); }
                throw new Error(`Request failed [${response.status}]`);
            }).then(parseFeatures).then(features => {
                if (!features.length) return;
                const ctx = canvas.getContext('2d');
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                for (const feature of features) {
                    const ringIndices = [0];
                    for (const ring of feature.geometry) {
                        ringIndices.push(ringIndices[ringIndices.length - 1] + ring.length);
                    }

                    const points = feature.geometry.flat(1)
                    const pointByteOffset = exports.allocArray(points.length);
                    const pointsView = new Float32Array(memory.buffer, pointByteOffset, points.length);
                    pointsView.set(points);

                    const idxByteOffset = exports.allocArray(ringIndices.length);
                    const idxView = new Uint32Array(memory.buffer, idxByteOffset, ringIndices.length);
                    idxView.set(ringIndices);

                    exports.plotFeature(
                        ctx, 
                        pointByteOffset, 
                        points.length, 
                        idxByteOffset, 
                        ringIndices.length, 
                        canvas.width, canvas.height,
                    );
                }
                temp.style.background = 'darkseagreen';
                if (selectedFeature) {
                    selectedFeature.style.background = 'transparent';
                }
                selectedFeature = temp;
            }).catch(error => {
                temp.style.backgroundColor = 'lightpink';
                console.error(`${featurePath}:\n${error}`);
            });
        };
    }
    
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

