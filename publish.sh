current=$(git rev-parse --abbrev-ref HEAD)
if [ "$current" != "master" ]; then
  git stash
  git checkout master
fi
cd app
wasm-pack build --target web --no-pack --out-name core --out-dir ../pkg
cd ..
rm "./pkg/.gitignore"
cp -r static/* pkg
cp -r js/* pkg
git add pkg/\*
if [ $# -eq 0 ]; then
    git commit -m "deployment from master"
else
  git commit -m "$1"
fi
git checkout gh-pages
git pull --rebase
git checkout master -- pkg/*
git checkout gh-pages
mv ./pkg/{.,}* ./
git add -A
git commit -m "deployment"
git push
git checkout $current
if [ "$current" != "master" ]; then
  git checkout $current
  git stash pop
fi