# We use the latest rust stable release as base image
FROM rust:1.63.0

# Let's switch our working directory to 'app' (equivalent to 'cd app')
# The 'app' folder will be created for us by Docker in case it does not
# exist already.

WORKDIR /app

# Install the required system dependencies for out linking configuration
RUN apt update && apt install lld clang -y

#Copy all files from out working environment to our Docker image
COPY . .

ENV SQLX_OFFLINE true
# Let's build our binary!
# We'll use the release profile to make it fast
RUN cargo build --release
# When 'docker run' is executed, launch the binary!
ENTRYPOINT ["./target/release/zero2Prod"]
