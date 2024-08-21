wasm-pack build --target web --no-pack --out-dir ./pkg
rm "./pkg/.gitignore"
cp -r static/* pkg
cp -r js/* pkg
git checkout gh-pages
git pull --rebase
git checkout master -- pkg/*
git add ./pkg/*
git commit -m "deployment"
git push
sleep 50