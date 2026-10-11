#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo 'Usage: verify-deb.sh PACKAGE EXPECTED_VERSION' >&2
    exit 1
fi

package=$1
version=$2
test "$(dpkg-deb -f "$package" Package)" = pomolice
test "$(dpkg-deb -f "$package" Version)" = "$version"
test "$(dpkg-deb -f "$package" Architecture)" = amd64

apt-get update
apt-get install -y --no-install-recommends "$package"
test "$(dpkg-query -W -f='${Version}' pomolice)" = "$version"
test -x /usr/bin/pomolice
file /usr/bin/pomolice | grep -q 'ELF 64-bit.*x86-64'
ldd /usr/bin/pomolice > /packages/shared-libraries.txt
if grep -q 'not found' /packages/shared-libraries.txt; then
    cat /packages/shared-libraries.txt >&2
    exit 1
fi

for resource in work short_break long_break; do
    cmp "/usr/lib/Pomolice/resources/sounds/$resource.wav" "/expected/$resource.wav"
done
cmp /usr/lib/Pomolice/THIRD-PARTY-NOTICES.md /expected/THIRD-PARTY-NOTICES.md
test -s /usr/share/doc/pomolice/copyright
test -s /usr/share/applications/Pomolice.desktop
test -s /usr/share/applications/io.github.NakuRei.pomolice.desktop
test -s /usr/share/icons/hicolor/128x128/apps/pomolice.png
desktop-file-validate /usr/share/applications/Pomolice.desktop
desktop-file-validate /usr/share/applications/io.github.NakuRei.pomolice.desktop
dbus-run-session -- sh /checks/verify-portal-registration.sh
if dpkg -L pomolice | grep -q export_tray_icon_fixtures; then
    echo 'Test fixture executable was included in the package.' >&2
    exit 1
fi
apt-get remove -y pomolice
test ! -e /usr/bin/pomolice
test ! -e /usr/share/applications/Pomolice.desktop
test ! -e /usr/share/applications/io.github.NakuRei.pomolice.desktop
