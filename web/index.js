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

const parseFeatures = data => {
    if (!('type' in data)) throw new Error();
    const type = data['type'];
    if (type != 'FeatureCollection') throw new Error();

    if (!('features' in data)) throw new Error();
    const features = [];
    for (const entry of data['features']) {
        if (!('properties' in entry)) continue;
        const properties = entry['properties'];

        if (!('NAME' in properties)) continue;
        const name = properties['NAME'];
        if (name == null) continue;

        if (!('geometry' in entry)) continue;
        const geometry = entry['geometry'];
        if (!('type' in geometry)) continue;
        if (geometry['type'] != 'MultiPolygon') continue;

        if (!('coordinates' in geometry)) continue;
        const coordinates = geometry['coordinates'];
        if (coordinates.length == 0) continue;

        const temp = {
            "name": name,
            "geometry": [],
        };

        for (const ring of coordinates) {
            temp.geometry.push(ring.flat(2));
        }

        features.push(temp);
    }

    if (features.length == 0) return null;
    return features;
};

const init = async _ => {
    const exports = await loadExports('core.wasm', 10);

    const canvas = document.querySelector('body > canvas');

    const featureList = document.getElementById('features');
    featureList.onwheel = event => {
        featureList.scrollLeft += event.deltaY;
        event.preventDefault();
    };

    for (const featurePath of exports.getFeatures()) {
        let temp = document.createElement('button');

        temp.appendChild(document.createTextNode(featurePath));
        temp.onclick = _ => {
            const url = `${location.href}${featurePath}`;
            fetch(url).then(response => {
                if (response.ok) {
                    return response.json();
                } else {
                    temp.style.backgroundColor = 'lightpink';
                    throw new Error(`Request failed [${response.status}]`);
                }
            }).then(data => {
                try {
                    const featureData = parseFeatures(data);
                    const ctx = canvas.getContext('2d');
                    ctx.clearRect(0, 0, canvas.width, canvas.height);
                    for (const feature of featureData) {
                        const points = feature.geometry.flat(1);
                        for (let i = 0; i < points.length / 2; i++) {
                            const x = points[i * 2];
                            const y = points[i * 2 + 1];
                            exports.plotPoint(ctx, x, y, canvas.width, canvas.height);
                        }
                    }
                    temp.style.background = 'transparent';
                } catch (err) {
                    throw new Error('Failed to parse feature layer');
                }
            }).catch(error => {
                temp.style.backgroundColor = 'lightpink';
                console.error(`${featurePath}:\n${error}`);
            });
        };

        featureList.appendChild(temp);
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

