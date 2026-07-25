import React, { useEffect, useState, useCallback } from "react";
import { Box, Text, useInput } from "ink";
import Spinner from "ink-spinner";
import { execScript, listInstalledApps, type InstalledApp } from "../utils/exec.js";

interface Props {
  onBack: () => void;
  onMessage: (msg: string | null) => void;
}

export default function UpdateScreen({ onBack, onMessage }: Props) {
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState(0);
  const [updating, setUpdating] = useState<string | null>(null);
  const [output, setOutput] = useState<string[]>([]);
  const [done, setDone] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onMessage("Select app to update");
    void (async () => {
      try {
        const data = await listInstalledApps();
        setApps(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    })();
  }, [onMessage]);

  const updateApp = useCallback(async (name: string) => {
    setUpdating(name);
    setOutput([]);
    setError(null);
    onMessage(`Updating ${name}...`);

    const result = await execScript(
      ["--update", name],
      (chunk) => setOutput((prev) => [...prev.slice(-30), chunk]),
      (chunk) => setOutput((prev) => [...prev.slice(-30), chunk]),
    );

    setUpdating(null);
    if (result.exitCode !== 0) {
      setError(`Failed to update ${name}: ${result.stderr || result.stdout}`);
    } else {
      setDone((prev) => [...prev, name]);
    }
  }, [onMessage]);

  useInput((input, key) => {
    if (updating) return;

    if (key.escape) {
      onBack();
    } else if (key.upArrow) {
      setSelected((s) => (s > 0 ? s - 1 : apps.length - 1));
    } else if (key.downArrow) {
      setSelected((s) => (s < apps.length - 1 ? s + 1 : 0));
    } else if (key.return) {
      const app = apps[selected];
      if (app) {
        void updateApp(app.name);
      }
    } else if (input.toLowerCase() === "a") {
      setUpdating("all");
      setOutput([]);
      setError(null);
      onMessage("Updating all apps...");
      void (async () => {
        const result = await execScript(
          ["--update-all"],
          (chunk) => setOutput((prev) => [...prev.slice(-30), chunk]),
          (chunk) => setOutput((prev) => [...prev.slice(-30), chunk]),
        );
        setUpdating(null);
        if (result.exitCode !== 0) {
          setError(`Update all failed: ${result.stderr || result.stdout}`);
        } else {
          setDone(apps.map((a) => a.name));
        }
      })();
    }
  });

  if (loading) {
    return (
      <Box>
        <Text color="cyan">
          <Spinner type="dots" />
        </Text>
        <Text> Loading apps...</Text>
      </Box>
    );
  }

  if (error && !updating) {
    return (
      <Box flexDirection="column">
        <Text color="red">Error: {error}</Text>
        <Text>Press Esc to go back</Text>
      </Box>
    );
  }

  if (updating) {
    return (
      <Box flexDirection="column">
        <Text color="cyan">
          <Spinner type="dots" /> Updating {updating === "all" ? "all apps" : updating}...
        </Text>
        <Box flexDirection="column" marginTop={1} borderStyle="single" padding={1} height={12}>
          {output.length === 0 ? (
            <Text dimColor>Waiting for output...</Text>
          ) : (
            output.map((line, i) => <Text key={i}>{line}</Text>)
          )}
        </Box>
      </Box>
    );
  }

  if (apps.length === 0) {
    return (
      <Box flexDirection="column">
        <Text>No installed apps.</Text>
        <Text dimColor>Press Esc to go back</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text bold>Update Apps</Text>
      <Text dimColor>Press [a] to update all, Esc to go back</Text>
      <Box flexDirection="column" marginTop={1}>
        {apps.map((app, index) => (
          <Box key={app.name}>
            <Text bold={index === selected} color={index === selected ? "cyan" : app.updatable ? "yellow" : undefined}>
              {index === selected ? "> " : "  "}
              {app.name}
            </Text>
            <Text> v{app.version}</Text>
            {app.updatable && <Text color="yellow"> [updatable]</Text>}
            {done.includes(app.name) && <Text color="green"> ✓</Text>}
          </Box>
        ))}
      </Box>
    </Box>
  );
}
