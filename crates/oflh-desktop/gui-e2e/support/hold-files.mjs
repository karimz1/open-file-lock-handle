// Fixture process for the GUI end-to-end tests: opens every path given on the
// command line (and, with --listen, a TCP port on 127.0.0.1), reports
// "ready <pid> [port <n>]" and keeps everything open until it is told to
// "release" on stdin or is stopped.
import fs from "node:fs";
import net from "node:net";
import readline from "node:readline";

const listen = process.argv.includes("--listen");
const paths = process.argv
  .slice(2)
  .filter((argument) => argument !== "--listen");
let handles = paths.map((file) => fs.openSync(file, "r"));
let server;

if (listen) {
  server = net.createServer((socket) => socket.end());
  server.listen(0, "127.0.0.1", () => {
    process.stdout.write(
      `ready ${process.pid} port ${server.address().port}\n`,
    );
  });
} else {
  process.stdout.write(`ready ${process.pid}\n`);
}

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  if (line.trim() === "release") {
    for (const handle of handles) fs.closeSync(handle);
    handles = [];
    const done = () => process.stdout.write("released\n");
    if (server) server.close(done);
    else done();
  }
});
// Keep running even when stdin closes.
setInterval(() => {}, 1 << 30);
