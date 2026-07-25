import React, { useEffect, useState } from "react";
import { Box, Text, useInput } from "ink";
import Spinner from "ink-spinner";
import { listInstalledApps, type InstalledApp } from "../utils/exec.js";

interface Props {
  onBack: () => void;
  onMessage: (msg: string | null) => void;
}

export default function ListScreen({ onBack, onMessage }: Props) {
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onMessage("Installed AppImages");
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

  useInput((_, key) => {
    if (key.escape || key.return) {
      onBack();
    }
  });

  if (loading) {
    return (
      <Box>
        <Text color="cyan">
          <Spinner type="dots" />
        </Text>
        <Text> Loading installed apps...</Text>
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
        <Text>No AppImages installed yet.</Text>
        <Text dimColor>Press Esc to go back</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text bold>Installed Apps ({apps.length})</Text>
      <Box flexDirection="column" marginTop={1}>
        {apps.map((app) => (
          <Box key={app.name}>
            <Text color={app.updatable ? "yellow" : "green"}>
              {app.updatable ? "↻ " : "  "}
              {app.name}
            </Text>
            <Text> — {app.displayName}</Text>
            <Text dimColor> v{app.version}</Text>
            <Text dimColor> ({app.size})</Text>
          </Box>
        ))}
      </Box>
      <Box marginTop={1}>
        <Text dimColor>Press Esc to go back</Text>
      </Box>
    </Box>
  );
}
