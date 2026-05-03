class UsereportRs < Formula
  desc "System performance analysis and reporting CLI"
  homepage "https://github.com/lukaspustina/usereport-rs"
  version "0.2.1"
  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.1/usereport-rs-aarch64-apple-darwin.tar.xz"
    sha256 "402a2b4f6adff7f05380401d47f927a98b46348d06845ca1aee56db59ff107f5"
  end
  if OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.1/usereport-rs-aarch64-unknown-linux-musl.tar.xz"
      sha256 "307e484377eb5e642472ec4d79a2088bb05738337213de9e28612bf9f58da35c"
    end
    if Hardware::CPU.intel?
      url "https://github.com/lukaspustina/usereport-rs/releases/download/v0.2.1/usereport-rs-x86_64-unknown-linux-musl.tar.xz"
      sha256 "a242e5afe2ffda5498320fcc9dbd74b1455cd066ce1b4b7b45e3132c7e252b47"
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
