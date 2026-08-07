import React, { useEffect, useState, useCallback } from "react";
import { Box, Text, useInput } from "ink";
import Spinner from "ink-spinner";
import { execScript, listInstalledApps, type InstalledApp } from "../utils/exec.js";

interface Props {
  onBack: () => void;
  onMessage: (msg: string | null) => void;
}

export default function RemoveScreen({ onBack, onMessage }: Props) {
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState(0);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onMessage("Select app to remove");
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

  const removeApp = useCallback(async (name: string) => {
    setRemoving(name);
    setError(null);
    onMessage(`Removing ${name}...`);

    try {
      const result = await execScript(["--remove", name]);
      if (result.exitCode !== 0) {
        setError(`Failed to remove ${name}: ${result.stderr || result.stdout}`);
      } else {
        setApps((prev) => prev.filter((a) => a.name !== name));
        setSelected((s) => Math.max(0, Math.min(s, apps.length - 2)));
        setConfirming(null);
      }
    } catch (err) {
      setError(`Failed to remove ${name}: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setRemoving(null);
    }
  }, [apps.length, onMessage]);

  useInput((_, key) => {
    if (removing) return;

    if (key.escape) {
      if (confirming) {
        setConfirming(null);
        onMessage("Select app to remove");
      } else {
        onBack();
      }
    } else if (key.upArrow && !confirming) {
      setSelected((s) => (s > 0 ? s - 1 : apps.length - 1));
    } else if (key.downArrow && !confirming) {
      setSelected((s) => (s < apps.length - 1 ? s + 1 : 0));
    } else if (key.return) {
      if (confirming) {
        void removeApp(confirming);
      } else {
        const app = apps[selected];
        if (!app) return;
        setConfirming(app.name);
        onMessage(`Press Enter again to remove ${app.name}, Esc to cancel`);
      }
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

  if (error) {
    return (
      <Box flexDirection="column">
        <Text color="red">Error: {error}</Text>
        <Text>Press Esc to go back</Text>
      </Box>
    );
  }

  if (apps.length === 0) {
    return (
      <Box flexDirection="column">
        <Text>No installed apps to remove.</Text>
        <Text dimColor>Press Esc to go back</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text bold>Remove App</Text>
      <Box flexDirection="column" marginTop={1}>
        {apps.map((app, index) => (
          <Box key={app.name}>
            <Text
              bold={index === selected}
              color={index === selected ? (confirming === app.name ? "red" : "cyan") : undefined}
            >
              {index === selected ? "> " : "  "}
              {app.name}
            </Text>
            <Text dimColor> v{app.version} ({app.size})</Text>
          </Box>
        ))}
      </Box>
      {confirming && (
        <Box marginTop={1}>
          <Text color="red" bold>
            Press Enter again to remove {confirming}, or Esc to cancel
          </Text>
        </Box>
      )}
    </Box>
  );
}
