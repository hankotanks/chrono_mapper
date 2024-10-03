cd runner
wasm-pack build --target web --no-pack --out-name core --out-dir ../pkg --features="logging"
cd ..
cp -r static/* pkg
cp -r js/* pkg
cp -r features pkg
miniserve pkg --index "index.html" -p 8080