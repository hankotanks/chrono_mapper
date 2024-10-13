if [ $# -lt 1 ]; then
  echo "The first argument must specify the target [native, wasm32]."
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
      if [ "$TARGET" = 'native' ]; then
        echo "Invalid argument: [--out, -o] is only valid when target is [wasm32]."
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
if [ "$TARGET" = 'native' ]; then 
  unset BACKEND_OUT_DIR
  cargo run --features="logging"
elif [ "$TARGET" = 'wasm32' ]; then
  if [ -z "$BACKEND_OUT_DIR" ]; then
      echo "Must specify the output location of the wasm package [--out, -o]."
      read -p "Press enter to exit."
      exit 2
  fi
  wasm-pack build --target web --no-pack --out-name core --out-dir $BACKEND_OUT_DIR --features="logging"
  miniserve $BACKEND_OUT_DIR --index "index.html" -p 8080
else
  echo "The first argument must specify the target [native, wasm32]."
  read -p "Press enter to exit."
  exit 2
fi
cd $TEMP