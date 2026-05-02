class UsereportRs < Formula
  desc "Collect system information for the first 60 seconds of a performance analysis"
  homepage "https://github.com/lukaspustina/usereport-rs"
  version "0.2.0"
  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.0/usereport-rs-aarch64-apple-darwin.tar.xz"
    sha256 "26fea778e606e5a6032ca196cbcc656afc1f39bdf44b1cd9790a1dead8a435de"
  end
  if OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.0/usereport-rs-aarch64-unknown-linux-musl.tar.xz"
      sha256 "123d9b36856f0bce08f99e8102662ab267d9c92ee699c5403a872fd7603eb8a4"
    end
    if Hardware::CPU.intel?
      url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.0/usereport-rs-x86_64-unknown-linux-musl.tar.xz"
      sha256 "37ecf581fb2ae397b38c38c267cd63e0488b2fa99c92f53484bbc393c392b5b5"
    end
  end
  license "MIT"

  BINARY_ALIASES = {
    "aarch64-apple-darwin":               {},
    "aarch64-unknown-linux-gnu":          {},
    "aarch64-unknown-linux-musl-dynamic": {},
    "aarch64-unknown-linux-musl-static":  {},
    "x86_64-unknown-linux-gnu":           {},
    "x86_64-unknown-linux-musl-dynamic":  {},
    "x86_64-unknown-linux-musl-static":   {},
  }.freeze

  def target_triple
    cpu = Hardware::CPU.arm? ? "aarch64" : "x86_64"
    os = OS.mac? ? "apple-darwin" : "unknown-linux-gnu"

    "#{cpu}-#{os}"
  end

  def install_binary_aliases!
    BINARY_ALIASES[target_triple.to_sym].each do |source, dests|
      dests.each do |dest|
        bin.install_symlink bin/source.to_s => dest
      end
    end
  end

  def install
    bin.install "usereport" if OS.mac? && Hardware::CPU.arm?
    bin.install "usereport" if OS.linux? && Hardware::CPU.arm?
    bin.install "usereport" if OS.linux? && Hardware::CPU.intel?

    install_binary_aliases!

    # Homebrew will automatically install these, so we don't need to do that
    doc_files = Dir["README.*", "readme.*", "LICENSE", "LICENSE.*", "CHANGELOG.*"]
    leftover_contents = Dir["*"] - doc_files

    # Install any leftover files in pkgshare; these are probably config or
    # sample files.
    pkgshare.install(*leftover_contents) unless leftover_contents.empty?
  end
end
