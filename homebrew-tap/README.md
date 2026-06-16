# homebrew-tap

Personal Homebrew tap for MonaDB.

Push this directory to `github.com/rchowell/homebrew-tap`, then:

```sh
brew tap rchowell/tap
brew install monadb
mona --version
```

After each GitHub Release, update `Formula/monadb.rb` version and `sha256` values
(see [`scripts/update-homebrew-sha256.sh`](../scripts/update-homebrew-sha256.sh) in the main repo).
