package main

import (
	"fmt"
	"os"

	"sandbox/internal/config"
	"sandbox/internal/result"
	"sandbox/internal/sandbox"
)

func main() {
	cfg, err := config.ParseFlags()
	if err != nil {
		res := result.New()
		res.SetInternalError(err.Error())
		res.Exit()
	}

	if cfg.IsChild {
		if childErr := sandbox.RunChild(cfg.ChildArgs); childErr != nil {
			fmt.Fprintf(os.Stderr, "child error: %v\n", childErr)
			os.Exit(1)
		}
		os.Exit(0)
	}

	sb := sandbox.New(cfg)
	res := sb.Run()

	res.Exit()
}
