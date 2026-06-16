# typed: false
# frozen_string_literal: true

# Install: brew tap rchowell/tap && brew install monadb
# Runs the `mona` interactive SQL shell (formula name monadb, binary name mona).
class Monadb < Formula
  desc "Embedded document database with a SQL shell"
  homepage "https://github.com/rchowell/MonaDB"
  license "Apache-2.0"
  version "0.1.0"

  BASE_URL = "https://github.com/rchowell/MonaDB/releases/download/v0.1.0"

  on_macos do
    on_arm do
      url "#{BASE_URL}/mona-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_AFTER_RELEASE"
    end
    on_intel do
      url "#{BASE_URL}/mona-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_AFTER_RELEASE"
    end
  end

  on_linux do
    on_arm do
      url "#{BASE_URL}/mona-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_AFTER_RELEASE"
    end
    on_intel do
      url "#{BASE_URL}/mona-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_AFTER_RELEASE"
    end
  end

  def install
    bin.install "mona"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mona --version")
  end
end
