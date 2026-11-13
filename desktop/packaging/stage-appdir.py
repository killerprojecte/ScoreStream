from pathlib import Path
import shutil,os,json,argparse,hashlib,subprocess
parser=argparse.ArgumentParser(description="Stage a Debian x86_64 validation AppDir from verified extracted runtime packages.")
parser.add_argument("--sysroot",required=True,type=Path)
parser.add_argument("--binary",required=True,type=Path)
parser.add_argument("--packages",required=True,type=Path)
parser.add_argument("--output",required=True,type=Path)
args=parser.parse_args()
src=args.sysroot.resolve(); dst=args.output.resolve(); lib=dst/'usr/lib/x86_64-linux-gnu'
if dst.exists(): raise SystemExit('Output must not exist; stage into a fresh directory.')
lib.mkdir(parents=True,exist_ok=True)
# Keep platform libc and its loader supplied by the supported base distribution.
exclude={'libc.so.6','libpthread.so.0','libdl.so.2','librt.so.1','libm.so.6','libresolv.so.2','libutil.so.1','libanl.so.1','ld-linux-x86-64.so.2'}
# Copy only helper/plugin trees used by the native WebView, not media playback plugins.
seeds=[args.binary]
for name in ['webkit2gtk-4.1','gtk-3.0','gdk-pixbuf-2.0','gio']:
 p=src/'usr/lib/x86_64-linux-gnu'/name
 if p.exists():
  shutil.copytree(p,lib/name,dirs_exist_ok=True,symlinks=False,ignore_dangling_symlinks=True)
  seeds.extend(f for f in p.rglob('*') if f.is_file() and f.read_bytes()[:4]==b'\x7fELF')
for name in ['bwrap','xdg-dbus-proxy','glib-compile-schemas']:
 p=src/'usr/bin'/name
 if p.exists():seeds.append(p)
# WebKit loads these GL entry points with dlopen, so they are not in ldd's graph.
for name in ['libGLESv2.so.2','libEGL.so.1','libGLX.so.0','libGLdispatch.so.0']:
 p=src/'usr/lib/x86_64-linux-gnu'/name
 if p.exists():
  shutil.copy2(p,lib/name,follow_symlinks=True);seeds.append(p)
env=os.environ.copy()
env['LD_LIBRARY_PATH']=str(src/'usr/lib/x86_64-linux-gnu')
# ldd is used only on this project's own binary and verified official Debian binaries.
import re
for seed in seeds:
 result=subprocess.run(['ldd',str(seed)],env=env,capture_output=True,text=True,check=True)
 if 'not found' in result.stdout:raise SystemExit(result.stdout)
 for line in result.stdout.splitlines():
  match=re.search(r'\s(\S+) => (/[^( ]+)',line)
  if not match:continue
  name,path=match.groups()
  if name in exclude:continue
  origin=src/'usr/lib/x86_64-linux-gnu'/name
  if not origin.exists():origin=src/path.lstrip('/')
  if not origin.exists():raise SystemExit(f'Unproven dependency outside verified sysroot: {path}')
  shutil.copy2(origin,lib/name,follow_symlinks=True)
for name in ['glib-2.0','mime','icons','fonts','fontconfig','themes']:
 p=src/'usr/share'/name
 if p.exists():shutil.copytree(p,dst/'usr/share'/name,dirs_exist_ok=True,symlinks=False,ignore_dangling_symlinks=True)
shutil.copytree(src/'usr/share/doc',dst/'usr/share/doc',dirs_exist_ok=True,symlinks=True)
bin=dst/'usr/bin';bin.mkdir(parents=True,exist_ok=True)
for name in ['bwrap','xdg-dbus-proxy','gdk-pixbuf-query-loaders','glib-compile-schemas']:
 p=src/'usr/bin'/name
 if p.exists():shutil.copy2(p,bin/name)
shutil.copy2(args.binary,bin/'scorestream-desktop')
# Tauri's maintained linuxdeploy GTK plugin uses this equal-size relocation.
for p in lib.glob('libwebkit*.so*'):
 b=p.read_bytes(); p.write_bytes(b.replace(b'/usr',b'././'))
shutil.copy2(args.packages,dst/'runtime-packages.json')
print(dst)

# Exact duplicates become relative symlinks; keep the bundle relocatable.
seen={}
for p in sorted(lib.rglob('*')):
 if p.is_file() and not p.is_symlink():
  digest=hashlib.sha256(p.read_bytes()).hexdigest()
  if digest in seen:
   target=seen[digest];p.unlink();p.symlink_to(os.path.relpath(target,p.parent))
  else:seen[digest]=p
shutil.copy2(Path(__file__).with_name('AppRun'),dst/'AppRun')
(dst/'AppRun').chmod(0o755)
repo=Path(__file__).resolve().parents[2]
shutil.copy2(repo/'crates/scorestream-desktop/icons/icon.png',dst/'ScoreStream.png')
(dst/'.DirIcon').symlink_to('ScoreStream.png')
(dst/'ScoreStream.desktop').write_text('[Desktop Entry]\nType=Application\nName=ScoreStream\nExec=scorestream-desktop\nIcon=ScoreStream\nCategories=AudioVideo;Audio;\nTerminal=false\n')
shutil.copytree(repo/'desktop/licenses',dst/'licenses',dirs_exist_ok=True)
shutil.copy2(repo/'desktop/THIRD_PARTY_NOTICES.md',dst/'THIRD_PARTY_NOTICES.md')
shutil.copy2(Path(__file__).with_name('README.md'),dst/'BUNDLE-NOTES.md')
compiler=bin/'glib-compile-schemas'
if compiler.exists():
 env=os.environ.copy();env['LD_LIBRARY_PATH']=str(lib)
 subprocess.run([str(compiler),str(dst/'usr/share/glib-2.0/schemas')],env=env,check=True)
files={str(p.relative_to(dst)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(dst.rglob('*')) if p.is_file()}
(dst/'SHA256SUMS.json').write_text(json.dumps(files,indent=2)+'\n')
