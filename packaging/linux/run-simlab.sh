#!/bin/sh
set -eu
suite_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
runtime_dir="$suite_dir/runtime"
export LD_LIBRARY_PATH="$suite_dir/lib"
export PATH="$runtime_dir/bin:$PATH"
export GTK_DATA_PREFIX="$runtime_dir/usr"
export GTK_PATH="$runtime_dir/usr/lib/x86_64-linux-gnu/gtk-3.0"
export GTK_THEME=Adwaita
export GSETTINGS_SCHEMA_DIR="$runtime_dir/usr/share/glib-2.0/schemas"
export GSETTINGS_BACKEND=memory
export GIO_MODULE_DIR="$runtime_dir/usr/lib/x86_64-linux-gnu/gio/modules"
export GI_TYPELIB_PATH="$runtime_dir/usr/lib/x86_64-linux-gnu/girepository-1.0"
export XDG_DATA_DIRS="$runtime_dir/usr/share"
export FONTCONFIG_PATH="$runtime_dir/etc/fonts"
export FONTCONFIG_FILE="$runtime_dir/etc/fonts/fonts.conf"
export GDK_PIXBUF_MODULEDIR="$runtime_dir/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0/2.10.0/loaders"
cache_root="${XDG_CACHE_HOME:-$HOME/.cache}/SimLab"
mkdir -p "$cache_root"
export GDK_PIXBUF_MODULE_FILE="$cache_root/pixbuf-loaders.cache"
"$runtime_dir/bin/gdk-pixbuf-query-loaders" "$GDK_PIXBUF_MODULEDIR"/*.so > "$GDK_PIXBUF_MODULE_FILE"
if [ -d "$runtime_dir/usr/lib/x86_64-linux-gnu/gstreamer-1.0" ]; then
  export GST_PLUGIN_SYSTEM_PATH_1_0="$runtime_dir/usr/lib/x86_64-linux-gnu/gstreamer-1.0"
  export GST_PLUGIN_SCANNER="$runtime_dir/bin/gst-plugin-scanner"
fi
# WebKitGTK's bundled helper/resource paths were rewritten to be relative.
cd "$runtime_dir/usr"
executable="$suite_dir/SAIO"
if [ "${1:-}" = --component ]; then
  role="${2:-}"
  case "$role" in
    modbus.master) binary=ModbusMaster ;;
    modbus.slave) binary=ModbusSlave ;;
    iec104.master) binary=iec104master-app ;;
    iec104.slave) binary=iec104sim-app ;;
    pmu.master) binary=pmusim-app ;;
    pmu.sub) binary=pmusim-sub ;;
    opcua.master) binary=opcuamaster-app ;;
    opcua.server) binary=opcuaserver-app ;;
    *) echo 'Unknown simulator role' >&2; exit 2 ;;
  esac
  executable="$suite_dir/apps/$role/$binary"
  shift 2
fi
exec "$executable" "$@"
