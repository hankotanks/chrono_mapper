cd app
wasm-pack build --target web --no-pack --out-name core --out-dir ../pkg
cd ..
rm "./pkg/.gitignore"
cp -r static/* pkg
cp -r js/* pkg
git add -f pkg/\*
git commit -m "temp"
git checkout gh-pages
git checkout master -- pkg/*
mv ./pkg/{.[!.],}* ./
git add -A
git commit -m "deployment"
git checkout master
git reset HEAD~