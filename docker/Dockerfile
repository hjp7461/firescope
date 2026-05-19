FROM node:20-bookworm-slim

# firebase-tools 14+ 는 Java 21+ 를 요구한다. Bookworm 기본 JRE는 17이므로
# Adoptium Temurin 21 JRE를 설치한다.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl gnupg \
 && mkdir -p /etc/apt/keyrings \
 && curl -fsSL https://packages.adoptium.net/artifactory/api/gpg/key/public \
      | gpg --dearmor -o /etc/apt/keyrings/adoptium.gpg \
 && echo "deb [signed-by=/etc/apt/keyrings/adoptium.gpg] https://packages.adoptium.net/artifactory/deb bookworm main" \
      > /etc/apt/sources.list.d/adoptium.list \
 && apt-get update && apt-get install -y --no-install-recommends temurin-21-jre \
 && rm -rf /var/lib/apt/lists/*

RUN npm install -g firebase-tools@latest

WORKDIR /workspace

EXPOSE 4000 8080 9099 9150

# 참고: docker-compose.yml이 entrypoint/command를 오버라이드한다
# (seed export 유무에 따라 --import 조건부 적용).
ENTRYPOINT ["firebase"]
CMD ["emulators:start", "--only", "firestore,auth", "--project", "demo-firescope"]
