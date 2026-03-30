FROM node:20-alpine AS base
RUN corepack enable && corepack prepare pnpm@9 --activate
WORKDIR /app

FROM base AS deps
COPY package.json pnpm-workspace.yaml ./
COPY core/package.json ./core/
COPY api/package.json ./api/
COPY cli/package.json ./cli/
RUN pnpm install --frozen-lockfile

FROM base AS build
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/core/node_modules ./core/node_modules
COPY --from=deps /app/api/node_modules ./api/node_modules
COPY . .
RUN pnpm build

FROM node:20-alpine AS runtime
WORKDIR /app
COPY --from=build /app/api/dist ./api/dist
COPY --from=build /app/core/dist ./core/dist
COPY --from=build /app/api/package.json ./api/
COPY --from=build /app/core/package.json ./core/
RUN corepack enable && corepack prepare pnpm@9 --activate
COPY package.json pnpm-workspace.yaml ./
RUN pnpm install --prod --frozen-lockfile

EXPOSE 3000
ENV NODE_ENV=production
CMD ["node", "api/dist/index.js"]
