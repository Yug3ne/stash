import React, { useState, useEffect, useCallback } from "react";
import { Box, Text, useInput, Spacer } from "ink";
import TextInput from "ink-text-input";
import { execScript } from "../utils/exec.js";

interface Props {
  onBack: () => void;
  onMessage: (msg: string | null) => void;
}

export default function InstallScreen({ onBack, onMessage }: Props) {
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [optimize, setOptimize] = useState(false);
  const [noSandbox, setNoSandbox] = useState(false);
  const [step, setStep] = useState<"path" | "options" | "confirm" | "running" | "done" | "error">("path");
  const [output, setOutput] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onMessage("Enter path to .AppImage file");
  }, [onMessage]);

  const handlePathSubmit = useCallback(() => {
    if (!path.trim()) return;
    setStep("options");
    onMessage("Configure options");
  }, [path, onMessage]);

  const runInstall = useCallback(async () => {
    setStep("running");
    onMessage("Installing...");
    setOutput([]);
    setError(null);

    const args = [path.trim()];
    if (name.trim()) args.push("--name", name.trim());
    if (optimize) args.push("--optimize");
    if (noSandbox) args.push("--no-sandbox");

    const result = await execScript(
      args,
      (chunk) => setOutput((prev) => [...prev.slice(-50), chunk]),
      (chunk) => setOutput((prev) => [...prev.slice(-50), chunk]),
    );

    if (result.exitCode !== 0) {
      setError(result.stderr || "Installation failed.");
      setStep("error");
    } else {
      setStep("done");
    }
  }, [path, name, optimize, noSandbox, onMessage]);

  useInput((input, key) => {
    if (step === "options") {
      if (input.toLowerCase() === "o") setOptimize((v) => !v);
      if (input.toLowerCase() === "n") setNoSandbox((v) => !v);
      if (key.return) {
        setStep("confirm");
        onMessage("Press Enter to install or Esc to go back");
      }
      if (key.escape) {
        setStep("path");
        onMessage("Enter path to .AppImage file");
      }
    } else if (step === "confirm") {
      if (key.return) {
        void runInstall();
      } else if (key.escape) {
        setStep("options");
        onMessage("Configure options");
      }
    } else if (step === "done" || step === "error") {
      if (key.escape || key.return) {
        setPath("");
        setName("");
        setOptimize(false);
        setNoSandbox(false);
        setOutput([]);
        setError(null);
        setStep("path");
        onMessage("Enter path to .AppImage file");
      }
    }
  });

  if (step === "path") {
    return (
      <Box flexDirection="column">
        <Text bold>Install AppImage</Text>
        <Box>
          <Text>Path: </Text>
          <TextInput value={path} onChange={setPath} onSubmit={handlePathSubmit} />
        </Box>
        <Box marginTop={1}>
          <Text dimColor>Press Enter to continue, Esc to go back</Text>
        </Box>
      </Box>
    );
  }

  if (step === "options") {
    return (
      <Box flexDirection="column">
        <Text bold>Install Options</Text>
        <Text>Path: {path}</Text>
        <Box>
          <Text>Custom name (optional): </Text>
          <TextInput value={name} onChange={setName} onSubmit={() => {}} />
        </Box>
        <Box marginTop={1}>
          <Text color={optimize ? "green" : undefined}>
            [{optimize ? "x" : " "}] [o] Optimize size/launch speed
          </Text>
        </Box>
        <Box>
          <Text color={noSandbox ? "green" : undefined}>
            [{noSandbox ? "x" : " "}] [n] --no-sandbox (Electron apps)
          </Text>
        </Box>
        <Box marginTop={1}>
          <Text dimColor>Press Enter to continue, Esc to go back</Text>
        </Box>
      </Box>
    );
  }

  if (step === "confirm") {
    return (
      <Box flexDirection="column">
        <Text bold>Confirm Installation</Text>
        <Text>Path: {path}</Text>
        {name && <Text>Name: {name}</Text>}
        <Text>Optimize: {optimize ? "yes" : "no"}</Text>
        <Text>No sandbox: {noSandbox ? "yes" : "no"}</Text>
        <Box marginTop={1}>
          <Text>Press Enter to install, Esc to edit options</Text>
        </Box>
      </Box>
    );
  }

  if (step === "running") {
    return (
      <Box flexDirection="column">
        <Text color="cyan">Installing...</Text>
        <Box flexDirection="column" marginTop={1} borderStyle="single" padding={1} height={12}>
          {output.length === 0 ? (
            <Text dimColor>Waiting for output...</Text>
          ) : (
            output.map((line, i) => <Text key={i}>{line}</Text>)
          )}
          <Spacer />
        </Box>
      </Box>
    );
  }

  if (step === "error") {
    return (
      <Box flexDirection="column">
        <Text color="red" bold>Installation failed</Text>
        <Text color="red">{error}</Text>
        <Box marginTop={1}>
          <Text>Press Enter or Esc to try again</Text>
        </Box>
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text color="green" bold>Installation complete!</Text>
      <Box marginTop={1}>
        <Text>Press Enter or Esc to return</Text>
      </Box>
    </Box>
  );
}
