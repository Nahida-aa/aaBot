import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "./openapi.json",
  output: "src/client",
  plugins: [
    {
      name: "@tanstack/solid-query",
      queryOptions: true,
    },
  ],
});
