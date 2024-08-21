wasm-pack build --target web --no-pack --out-di   r ./pkg
rm "./pkg/.gitignore"
cp -r "./static"* "./pkg"
cp -r "./js"* "./pkg"
git switch --orphan gh-pages
git checkout gh-pages
git checkout master -- pkg
git add ./pkg
git commit -m "deployment"