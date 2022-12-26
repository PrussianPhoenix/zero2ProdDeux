FROM lukemathwalker/cargo-chef:latest-rust-1.63.0 as chef

# Let's switch our working directory to 'app' (equivalent to 'cd app')
# The 'app' folder will be created for us by Docker in case it does not
# exist already.

WORKDIR /app

# Install the required system dependencies for out linking configuration
RUN apt update && apt install lld clang -y

FROM chef as planner
#Copy all files from our working environment to our Docker image
COPY . .

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
#Build our project dependencies, not the application
RUN cargo chef cook --release --recipe-path recipe.json
#if dependency tree stays the same, all layers should be cached
COPY . .

ENV SQLX_OFFLINE true
# Let's build our binary!
# We'll use the release profile to make it fast
RUN cargo build --release --bin zero2prod

FROM debian:bullseye-slim AS runtime

WORKDIR /app

# lets install openssl as it is dynamicly-linked to some of our dependancies
# install ca-certificates to verify tls certificates
# when establishing https connections
run apt-get update -y \
 && apt-get install -y --no-install-recommends openssl ca-certificates\
 # cleanup
 && apt-get autoremove -y \
 && apt-get clean -y \
 && rm -rf /var/lib/apt/lists/*

#copy the compiled binary from the builder environment
# to our runtime environment
COPY --from=builder /app/target/release/zero2prod zero2prod
#we need the configuration file at runtime!
COPY configuration configuration
ENV APP_ENVIRONMENT production
# When 'docker run' is executed, launch the binary!
ENTRYPOINT ["./zero2prod"]
