if [ $# -lt 1 ]; then
  echo "The first argument must specify the target [native-run, wasm32-host, wasm32-publish]."
  read -p "Press enter to exit."
  exit 2
else
  TARGET="$1"
  shift
fi
while [[ $# -gt 0 ]]; do
  case $1 in
    -c|--crate)
      export CRATE_DIR=$(realpath $2)
      if [[ "$CRATE_DIR" =~ ^\/[a-z]\/.* ]]; then
        export CRATE_DIR="${CRATE_DIR:1:1}:${CRATE_DIR:2}"
      fi
      shift
      shift
      ;;
    -o|--out)
      if [ "$TARGET" = 'native-run' ]; then
        echo "Invalid argument: [--out, -o] is only valid when target is [wasm32-host, wasm32-publish]."
        read -p "Press enter to exit."
        exit 2
      fi
      export BACKEND_OUT_DIR=$(realpath $2)
      if [[ "$BACKEND_OUT_DIR" =~ ^\/[a-z]\/.* ]]; then
        export BACKEND_OUT_DIR="${BACKEND_OUT_DIR:1:1}:${BACKEND_OUT_DIR:2}"
      fi
      shift
      shift
      ;;
    -s|--static)
      export BACKEND_STATIC_ASSETS_DIR=$(realpath $2)
      if [[ "$BACKEND_STATIC_ASSETS_DIR" =~ ^\/[a-z]\/.* ]]; then
        export BACKEND_STATIC_ASSETS_DIR="${BACKEND_STATIC_ASSETS_DIR:1:1}:${BACKEND_STATIC_ASSETS_DIR:2}"
      fi
      shift
      shift
      ;;
    -l|--local)
      export BACKEND_LOCAL_ASSETS_DIR=$(realpath $2)
      if [[ "$BACKEND_LOCAL_ASSETS_DIR" =~ ^\/[a-z]\/.* ]]; then
        export BACKEND_LOCAL_ASSETS_DIR="${BACKEND_LOCAL_ASSETS_DIR:1:1}:${BACKEND_LOCAL_ASSETS_DIR:2}"
      fi
      shift
      shift
      ;;
    -p|--port)
      if ! [[ "$cms" =~ ^(native-run|wasm32-publish)$ ]]; then
        echo "Port can only be specified with target [wasm32-host]"
        read -p "Press enter to exit."
        exit 2
      fi
      BACKEND_PORT="$2"
      shift
      shift
      ;;
    -*|--*)
      echo "Unknown parameter provided: $1."
      read -p "Press enter to exit."
      exit 2
      ;;
  esac
done
if [ -z "$CRATE_DIR" ]; then
  echo "Must specify a crate containing a binary [--crate, -c]."
  read -p "Press enter to exit."
  exit 2
fi
if [ -z "$BACKEND_STATIC_ASSETS_DIR" ]; then
  echo "A static assets folder is required (even if it's empty) [--static, -s]."
  read -p "Press enter to exit."
  exit 2
fi
TEMP=$(pwd)
cd $CRATE_DIR
if [ "$TARGET" = 'native-run' ]; then 
  unset BACKEND_OUT_DIR
  cargo run --features="logging"
else
  if [ -z "$BACKEND_OUT_DIR" ]; then
    echo "Must specify the output location of the wasm package [--out, -o]."
    read -p "Press enter to exit."
    exit 2
  fi
  wasm-pack build --target web --no-pack --out-name core --out-dir $BACKEND_OUT_DIR --features="logging"
  if [ "$TARGET" = 'wasm32-host' ]; then
    if [ -z "$BACKEND_PORT" ]; then
      BACKEND_PORT="8080"
    fi
    miniserve $BACKEND_OUT_DIR --index "index.html" -p $BACKEND_PORT
  elif [ "$TARGET" = 'wasm32-publish' ]; then
    git diff-files --quiet
    BACKEND_DIFF_EXIT_CODE=$(echo $?)
    if [ "$BACKEND_DIFF_EXIT_CODE" = "1" ] || [ "$BACKEND_DIFF_EXIT_CODE" = "False" ]; then
      echo "Unable to publish the package when there are uncommited changes in the current branch."
      read -p "Press enter to exit."
      exit 2
    fi
    cd $BACKEND_OUT_DIR
    cd ..
    BACKEND_OUT_DIR_PARENT=$(pwd)
    git add -f pkg/\*
    git commit -m.
    git checkout gh-pages
    cd $BACKEND_OUT_DIR_PARENT
    git checkout $BACKEND_CURR_BRANCH -- ./pkg/*
    BACKEND_TOP_LEVEL=$(git rev-parse --show-toplevel)
    cp -a ./pkg/. $BACKEND_TOP_LEVEL
    cd pkg
    rm ".gitignore"
    BACKEND_FILES_TO_COMMIT=()
    for TEMP_FILENAME in *; do
      BACKEND_FILES_TO_COMMIT+=($TEMP_FILENAME)
    done
    cd $BACKEND_TOP_LEVEL
    for TEMP_FILENAME in "${BACKEND_FILES_TO_COMMIT[@]}"
    do
      git add -f $TEMP_FILENAME
    done
    git commit -m.
    git checkout $BACKEND_CURR_BRANCH
    git reset HEAD~1
  else
    echo "The first argument must specify the target [native-run, wasm32-host, wasm32-publish]."
    read -p "Press enter to exit."
    exit 2
  fi
fi
cd $TEMP
read -p "Press enter to exit."