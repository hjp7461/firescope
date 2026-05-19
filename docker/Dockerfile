FROM node:20-bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
      default-jre-headless \
      ca-certificates \
      curl \
 && rm -rf /var/lib/apt/lists/*

RUN npm install -g firebase-tools@latest

WORKDIR /workspace

EXPOSE 4000 8080 9099 9150

ENTRYPOINT ["firebase"]
CMD ["emulators:start", "--only", "firestore,auth", "--project", "demo-firescope"]
