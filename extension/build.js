const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { execSync } = require('child_process');

const root = __dirname;
const distRoot = path.join(root, 'dist');
const distChromium = path.join(distRoot, 'chromium');
const distChromiumStore = path.join(distRoot, 'chromium-store');
const distFirefox = path.join(distRoot, 'firefox');
const tempDir = path.join(distRoot, 'temp');
const tempSourceDir = path.join(distRoot, 'temp-source');

// Ensure clean dist folders
fs.rmSync(distRoot, { recursive: true, force: true });
fs.mkdirSync(distChromium, { recursive: true });
fs.mkdirSync(distChromiumStore, { recursive: true });
fs.mkdirSync(distFirefox, { recursive: true });
fs.mkdirSync(tempDir, { recursive: true });
fs.mkdirSync(tempSourceDir, { recursive: true });

console.log('[EDM] Transpiling extension scripts with esbuild (minify: false)...');
try {
  let esbuild;
  try {
    esbuild = require('esbuild');
  } catch (_e) {
    esbuild = require('../apps/desktop/node_modules/esbuild');
  }
  esbuild.buildSync({
    entryPoints: [
      path.join(root, 'src', 'background.ts'),
      path.join(root, 'src', 'content.ts'),
      path.join(root, 'src', 'popup.ts'),
      path.join(root, 'src', 'options.ts'),
      path.join(root, 'src', 'yt-main.ts')
    ],
    outdir: tempDir,
    target: 'es2020',
    bundle: true,
    minify: false
  });
  console.log('[EDM] esbuild transpile successful.');
} catch (e) {
  console.error('[EDM] esbuild failed:', e);
  process.exit(1);
}

function copyCommonAssets(targetDir, includeYtMain) {
  const htmlCssAssets = ['popup.html', 'popup.css', 'options.html', 'logo.png'];
  for (const file of htmlCssAssets) {
    const srcPath = path.join(root, 'src', file);
    if (fs.existsSync(srcPath)) {
      fs.copyFileSync(srcPath, path.join(targetDir, file));
    }
  }

  const scripts = ['background.js', 'content.js', 'popup.js', 'options.js'];
  if (includeYtMain) scripts.push('yt-main.js');
  for (const script of scripts) {
    const builtPath = path.join(tempDir, script);
    if (fs.existsSync(builtPath)) {
      fs.copyFileSync(builtPath, path.join(targetDir, script));
    }
  }
}

function computeHashes(targetDir, manifestVersion, filesList) {
  const hashes = {
    version: manifestVersion || '0.1.1',
    generated_at: new Date().toISOString(),
    files: {}
  };
  for (const file of filesList) {
    const filePath = path.join(targetDir, file);
    if (fs.existsSync(filePath)) {
      const fileBuf = fs.readFileSync(filePath);
      const hashHex = crypto.createHash('sha256').update(fileBuf).digest('hex');
      hashes.files[file] = `sha256:${hashHex}`;
    }
  }
  const hashesJson = JSON.stringify(hashes, null, 2);
  fs.writeFileSync(path.join(targetDir, 'hashes.json'), hashesJson);
  return hashes;
}

function zipDirectory(sourceDir, outZipPath) {
  if (fs.existsSync(outZipPath)) fs.unlinkSync(outZipPath);
  if (process.platform === 'win32') {
    execSync(
      `powershell -NoProfile -Command "Compress-Archive -Path '${sourceDir}\\*' -DestinationPath '${outZipPath}' -Force"`,
      { stdio: 'ignore' }
    );
  } else {
    execSync(`cd "${sourceDir}" && zip -r "${outZipPath}" ./*`, { stdio: 'ignore' });
  }
}

// 1. Chromium unpacked/dev build (dist/chromium)
const chromiumManifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.chromium.json'), 'utf8'));
fs.writeFileSync(path.join(distChromium, 'manifest.json'), JSON.stringify(chromiumManifest, null, 2));
copyCommonAssets(distChromium, true);

const allChromiumFiles = [
  'manifest.json', 'background.js', 'content.js', 'popup.html',
  'popup.css', 'popup.js', 'options.html', 'options.js', 'yt-main.js', 'logo.png'
];
const chromiumHashes = computeHashes(distChromium, chromiumManifest.version, allChromiumFiles);

// Sync unpacked Chromium bundle to extension/ root so loading extension/ directly works
for (const file of [...allChromiumFiles, 'hashes.json']) {
  const srcFile = path.join(distChromium, file);
  if (fs.existsSync(srcFile)) {
    fs.copyFileSync(srcFile, path.join(root, file));
  }
}

// 2. Chromium Web Store & Edge Add-ons store build (strips "key" and yt-main.js / *://*.youtube.com/*)
const storeManifest = JSON.parse(JSON.stringify(chromiumManifest));
delete storeManifest.key;
if (Array.isArray(storeManifest.content_scripts)) {
  storeManifest.content_scripts = storeManifest.content_scripts.filter(
    (cs) => !(Array.isArray(cs.js) && cs.js.includes('yt-main.js')) &&
            !(Array.isArray(cs.matches) && cs.matches.some((m) => m.includes('youtube.com')))
  );
}
if (Array.isArray(storeManifest.web_accessible_resources)) {
  storeManifest.web_accessible_resources = storeManifest.web_accessible_resources.filter(
    (war) => !(Array.isArray(war.resources) && war.resources.includes('yt-main.js')) &&
             !(Array.isArray(war.matches) && war.matches.some((m) => m.includes('youtube.com')))
  );
  if (storeManifest.web_accessible_resources.length === 0) {
    delete storeManifest.web_accessible_resources;
  }
}
fs.writeFileSync(path.join(distChromiumStore, 'manifest.json'), JSON.stringify(storeManifest, null, 2));
copyCommonAssets(distChromiumStore, false);
computeHashes(distChromiumStore, storeManifest.version, allChromiumFiles.filter((f) => f !== 'yt-main.js'));

// 3. Firefox AMO build (dist/firefox)
const firefoxManifest = JSON.parse(fs.readFileSync(path.join(root, 'manifest.firefox.json'), 'utf8'));
fs.writeFileSync(path.join(distFirefox, 'manifest.json'), JSON.stringify(firefoxManifest, null, 2));
copyCommonAssets(distFirefox, false);
computeHashes(distFirefox, firefoxManifest.version, allChromiumFiles.filter((f) => f !== 'yt-main.js'));

// 4. Package Chromium store zip (for Chrome Web Store & Edge Add-ons)
const zipPath = path.join(distRoot, 'edm-extension-chromium.zip');
try {
  zipDirectory(distChromiumStore, zipPath);
  console.log(`[EDM] Packaged Chromium store zip: ${path.relative(root, zipPath)}`);
} catch (e) {
  console.warn(`[EDM] Chromium store zip creation warning: ${e.message}`);
}

// 5. Package Firefox XPI (for web-ext sign or AMO upload)
const xpiZipPath = path.join(distRoot, 'edm-firefox.zip');
const xpiDistPath = path.join(distRoot, 'edm-firefox.xpi');
const xpiTargetInFirefox = path.join(distFirefox, 'edm-firefox.xpi');
try {
  if (fs.existsSync(xpiDistPath)) fs.unlinkSync(xpiDistPath);
  if (fs.existsSync(xpiTargetInFirefox)) fs.unlinkSync(xpiTargetInFirefox);

  zipDirectory(distFirefox, xpiZipPath);
  if (fs.existsSync(xpiZipPath)) {
    fs.renameSync(xpiZipPath, xpiDistPath);
  }
  if (fs.existsSync(xpiDistPath)) {
    fs.copyFileSync(xpiDistPath, xpiTargetInFirefox);
  }
  console.log(`[EDM] Packaged Firefox XPI: ${path.relative(root, xpiDistPath)}`);
} catch (e) {
  console.warn(`[EDM] Firefox XPI creation warning: ${e.message}`);
}

// 6. Package clean Firefox AMO source zip (for AMO reviewers)
const sourceZipPath = path.join(distRoot, 'edm-firefox-source.zip');
try {
  const sourceFiles = [
    'package.json',
    'package-lock.json',
    'build.js',
    'manifest.firefox.json',
    'manifest.chromium.json',
    'BUILD.md'
  ];
  for (const file of sourceFiles) {
    const srcPath = path.join(root, file);
    if (fs.existsSync(srcPath)) {
      fs.copyFileSync(srcPath, path.join(tempSourceDir, file));
    }
  }
  fs.cpSync(path.join(root, 'src'), path.join(tempSourceDir, 'src'), { recursive: true });
  zipDirectory(tempSourceDir, sourceZipPath);
  console.log(`[EDM] Packaged Firefox AMO source zip: ${path.relative(root, sourceZipPath)}`);
} catch (e) {
  console.warn(`[EDM] Firefox source zip creation warning: ${e.message}`);
}

// Clean up temporary staging directories
fs.rmSync(tempDir, { recursive: true, force: true });
fs.rmSync(distChromiumStore, { recursive: true, force: true });
fs.rmSync(tempSourceDir, { recursive: true, force: true });
console.log(`[EDM] Extension built successfully! Computed hashes for ${Object.keys(chromiumHashes.files).length} files.`);
