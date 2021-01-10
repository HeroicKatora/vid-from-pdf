Name:           vid-from-pdf
Version:        1.0.0
%define         oversion 1.0.0-beta.3
Release:        4%{?dist}
Summary:        Create a simple video from PDF slides and audio
License:        AGPL-3.0-or-later
URL:            https://github.com/HeroicKatora/vid-from-pdf
# Note: manually download into rpmbuild/SOURCES with
# wget --content-disposition https://github.com/HeroicKatora/vid-from-pdf/archive/%{oversion}.tar.gz
Source:         vid-from-pdf-%{oversion}.tar.gz

# To build on a different host system without any requirements:
# rpmbuild -bb --nodeps
# The choice of cargo should not affect the usability of the end result and
# there are no dynamically linked dependencies after this build process.
BuildRequires:  cargo

%description
Create a simple video from PDF slides and audio

This is not a full-features video editor, it instead focusses on simplicity and
provides a wrapper around ffmpeg and mupdf.

%prep
%setup -n "%{name}-%{oversion}"

%build
cd $RPM_BUILD_DIR/%{name}-%{oversion}
cargo build --release --target x86_64-unknown-linux-musl

%install
rm -rf $RPM_BUILD_ROOT
install -D -m 755 target/x86_64-unknown-linux-musl/release/vid-from-pdf $RPM_BUILD_ROOT/opt/vid-from-pdf/vid-from-pdf
install -D -m 644 vid-from-pdf.desktop $RPM_BUILD_ROOT/usr/share/applications/vid-from-pdf.desktop

%files
/opt/vid-from-pdf/vid-from-pdf
/usr/share/applications/vid-from-pdf.desktop
