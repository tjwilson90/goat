import process from "process";

import esbuild from "esbuild";
import { copy } from "esbuild-plugin-copy";

const mode = process.argv[2] ?? "build";

const context = await esbuild.context({
  entryPoints: ['src/app.tsx'],
  bundle: true,
  minify: true,
  treeShaking: true,
  sourcemap: true,
  format: "esm",
  outfile: 'build/goat.js',
  logLevel: "info",
  plugins: [
    copy({
      assets: {
        from: ['./static/*'],
        to: ['./'] // Implicitly adds 'build' to all paths.
      },
      watch: mode == "watch"
    })
  ]
});

if (mode == "watch") {
  await context.watch();
  await context.serve({ servedir: 'build', port: 3000 });

  console.log("Serving build on http://localhost:3000/.")
} else {
  await context.rebuild();
  context.dispose();
}