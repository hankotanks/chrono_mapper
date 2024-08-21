current=$(git rev-parse --abbrev-ref HEAD)
if [ "$current" != "master" ]; then
  git stash
  git checkout master
fi
wasm-pack build --target web --no-pack --out-dir ./pkg
rm "./pkg/.gitignore"
cp -r static/* pkg
cp -r js/* pkg
git add -f pkg/\*
git commit -m "deployment from master"
git checkout gh-pages
git pull --rebase
git checkout master -- pkg/*
mv ./pkg/{.,}* ./
rmdir ./pkg
git add *
git commit -m "deployment"
git push
git checkout $current
git stash pop