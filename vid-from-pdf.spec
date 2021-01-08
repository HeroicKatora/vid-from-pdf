Name:           vid-from-pdf
Version:        1.0.0
Release:        1%{?dist}
Summary:        Create a simple video from PDF slides and audio
License:        AGPL-3.0-or-later
URL:            https://github.com/HeroicKatora/vid-from-pdf
Source:         vid-from-pdf-v1.0.0-beta.tar.gz

BuildRequires:  cargo

%description
Create a simple video from PDF slides and audio

This is not a full-features video editor, it instead focusses on simplicity and
provides a wrapper around ffmpeg and mupdf.

%prep
%setup -n "vid-from-pdf-1.0.0-beta"

%build
cd $RPM_BUILD_DIR/%{name}-%{version}-beta
cargo build --release --target x86_64-unknown-linux-musl

%install
rm -rf $RPM_BUILD_ROOT
install -D -m 755 target/x86_64-unknown-linux-musl/release/vid-from-pdf $RPM_BUILD_ROOT/opt/vid-from-pdf/vid-from-pdf
install -D -m 644 vid-from-pdf.desktop $RPM_BUILD_ROOT/usr/share/applications/vid-from-pdf.desktop

%files
/opt/vid-from-pdf/vid-from-pdf
/usr/share/applications/vid-from-pdf.desktop
