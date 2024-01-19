prod:
  cargo run --release -- --duco-addr=192.168.1.39 --mqtt-addr=192.168.1.13 --mqtt-base-topic home/ventilation

docker:
  docker build -t dirkvdb/duco2mqtt:latest -f docker/BuildDockerfile .

dockerup:
  docker push dirkvdb/duco2mqtt:latest
