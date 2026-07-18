package main

import (
	"os"

	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/cli"
)

func main() {
	os.Exit(cli.Run(os.Args[1:], os.Stdout, os.Stderr))
}
