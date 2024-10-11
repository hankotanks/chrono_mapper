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
      export BACKEND_OUT_DIR=$(realpath $2)
      if [[ "$BACKEND_OUT_DIR" =~ ^\/[a-z]\/.* ]]; then
        export BACKEND_OUT_DIR="${BACKEND_OUT_DIR:1:1}:${BACKEND_OUT_DIR:2}"
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
if [ -z "$BACKEND_OUT_DIR" ]; then
  echo "Must specify package location [--out, -o]."
  read -p "Press enter to exit."
  exit 2
fi
if [ -z "$CRATE_DIR" ]; then
  echo "Must provide the location of the crate to pack [--crate, -c]."
  read -p "Press enter to exit."
  exit 2
fi
TEMP=$(pwd)
cd $CRATE_DIR
wasm-pack build --target web --no-pack --out-name core --out-dir $BACKEND_OUT_DIR --features="logging"
cd $TEMP
miniserve $BACKEND_OUT_DIR --index "index.html" -p 8080