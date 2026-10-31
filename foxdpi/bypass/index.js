#!/usr/bin/env node
//
// foxdpi-bypass — kendi DPI bypass motorumuz (şablon başlangıcı)
//
// Kullanım:
//   node foxdpi-bypass/index.js
//   FOXDPI_ENGINE_DIR=. node foxdpi-bypass/index.js
//   FOXDPI_ENGINE_DIR=. FOXDPI_BYPASS_MODE=split node foxdpi-bypass/index.js
//
// Bu dosya şu an sadece yerel TCP sunucusu başlatır ve bağlantıları loglar.
// Gerçek bypass motoru için:
//   1) ilk TCP/TLS ClientHello paketini parse edecek düşük-seviye katman,
//   2) TLS record split / segment desync / decoy gibi strateji motoru,
//   3) yerel SOCKS5/HTTP proxy veya doğrudan TCP forwarder,
//   4) Linux sistem proxy/PAC ayarlarını yönetecek dağıtım katmanı,
//   5) deb/rpm/appimage/tar.gz/AUR/script ile kurulabilen daemon/package.
//
// Şablon olarak aşağıdaki sadece sunucu başlatıp loglar.
"use strict";

const net = require("node:net");

const PORT = Number(process.env.FOXDPIPORT || 10850);
const MODE = process.env.FOXDPI_BYPASS_MODE || "default";

function log(level, msg) {
  const ts = new Date().toISOString();
  console.error(`[foxdpi-bypass][${ts}][${level}] ${msg}`);
}

function main(argv) {
  log("info", `foxdpi kendi DPI bypass motoru (şablon) — mode=${MODE}`);
  log("info", `dinleme: 127.0.0.1:${PORT}`);

  const server = net.createServer((socket) => {
    const peer = `${socket.remoteAddress}:${socket.remotePort}`;
    log("info", `yeni bağlantı: ${peer} mode=${MODE}`);

    socket.on("data", (chunk) => {
      // TODO: ilk payload'ı incele, SNI/Host pozisyonunu bul,
      // TODO: strateji uygula (record split, segment desync, decoy, vs.)
      log("debug", `<- ${peer} ${chunk.length}B`);
    });

    socket.on("close", () => {
      log("info", `kapandı: ${peer}`);
    });

    socket.on("error", (err) => {
      log("error", `bağlantı hatası (${peer}): ${err && err.message}`);
    });
  });

  server.on("error", (err) => {
    log("error", `sunucu hatası: ${err && err.message}`);
    process.exit(1);
  });

  server.on("listening", () => {
    log("info", `hazır. 127.0.0.1:${PORT}`);
  });

  server.listen(PORT, "127.0.0.1");
}

// Bu dosya doğrudan çalıştırılırsa veya require edilip .start() çağrılınca çalışsın.
if (require.main === module) {
  main(process.argv.slice(2));
} else if (typeof module !== "undefined" && module.exports) {
  module.exports = { start: main, mode: MODE };
}
