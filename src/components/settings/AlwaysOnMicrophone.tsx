import React from "react";
import { useTranslation } from "react-i18next";
import type { MicWarmMode } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { Select, type SelectOption } from "../ui/Select";
import { SettingContainer } from "../ui/SettingContainer";

interface AlwaysOnMicrophoneProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AlwaysOnMicrophone: React.FC<AlwaysOnMicrophoneProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const micWarmMode = (getSetting("mic_warm_mode") ??
      (getSetting("always_on_microphone") ? "always" : "off")) as MicWarmMode;

    const options = React.useMemo<SelectOption[]>(
      () => [
        {
          value: "off",
          label: t("settings.debug.micWarmMode.options.off"),
        },
        {
          value: "timed",
          label: t("settings.debug.micWarmMode.options.timed"),
        },
        {
          value: "always",
          label: t("settings.debug.micWarmMode.options.always"),
        },
      ],
      [t],
    );

    return (
      <SettingContainer
        title={t("settings.debug.micWarmMode.title")}
        description={t("settings.debug.micWarmMode.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Select
          value={micWarmMode}
          options={options}
          onChange={(value) => {
            if (!value) return;
            void updateSetting("mic_warm_mode", value as MicWarmMode);
          }}
          disabled={isUpdating("mic_warm_mode")}
          isClearable={false}
          className="min-w-[220px]"
        />
      </SettingContainer>
    );
  },
);
