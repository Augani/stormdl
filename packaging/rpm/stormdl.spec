Name:           stormdl
Version:        0.1.0
Release:        1%{?dist}
Summary:        Next-generation download accelerator

License:        MIT
URL:            https://github.com/augustusotu/stormdl
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.70
BuildRequires:  cargo
BuildRequires:  openssl-devel

%description
StormDL is a next-generation download accelerator that uses adaptive
multi-segment parallel downloads to saturate available bandwidth.

Features:
- HTTP/1.1, HTTP/2, HTTP/3 (QUIC) support
- Adaptive segment splitting based on bandwidth-delay product
- Multi-source/mirror downloads
- Resume support with integrity verification
- Terminal UI with per-segment progress

%prep
%autosetup

%build
cargo build --release --locked

%install
install -Dm755 target/release/storm %{buildroot}%{_bindir}/storm
install -Dm644 config/default.toml %{buildroot}%{_sysconfdir}/storm-dl/config.toml

%files
%license LICENSE
%{_bindir}/storm
%config(noreplace) %{_sysconfdir}/storm-dl/config.toml

%changelog
* %(date "+%a %b %d %Y") Augustus Otu <augustus@example.com> - 0.1.0-1
- Initial package
