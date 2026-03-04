import React from "react";
import { useTranslation } from "react-i18next";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { ShortcutInput } from "../ShortcutInput";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { PushToTalk } from "../PushToTalk";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { VolumeSlider } from "../VolumeSlider";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { ModelSettingsCard } from "./ModelSettingsCard";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Select, type SelectOption } from "../../ui/Select";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import type { TranscriptionBackend } from "@/bindings";

const CLOUD_STT_DEFAULT_PROVIDER_ID = "groq";
const CLOUD_STT_MAX_AUDIO_SECONDS = [60, 120, 180, 300];
const CLOUD_STT_TIMEOUT_SECONDS = [30, 60, 90, 120, 180, 300];

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    settings,
    getSetting,
    updateSetting,
    isUpdating,
    audioFeedbackEnabled,
    updateCloudSttBaseUrl,
    updateCloudSttApiKey,
    updateCloudSttModel,
    fetchCloudSttModels,
    cloudSttModelOptions,
  } = useSettings();

  const transcriptionBackend = getSetting("transcription_backend") ?? "local";
  const cloudProviderId =
    settings?.cloud_stt_provider_id ?? CLOUD_STT_DEFAULT_PROVIDER_ID;

  const cloudApiKey = settings?.cloud_stt_api_keys?.[cloudProviderId] ?? "";
  const cloudModel = settings?.cloud_stt_models?.[cloudProviderId] ?? "";
  const cloudBaseUrl = settings?.cloud_stt_base_url?.[cloudProviderId] ?? "";
  const cloudFallbackToLocal =
    getSetting("cloud_stt_fallback_to_local") ?? true;
  const cloudPreloadLocalModel =
    getSetting("cloud_stt_preload_local_model") ?? false;
  const cloudMaxAudioSeconds = getSetting("cloud_stt_max_audio_seconds") ?? 180;
  const cloudRequestTimeoutSeconds =
    getSetting("cloud_stt_request_timeout_seconds") ?? 90;

  const [apiKeyInput, setApiKeyInput] = React.useState(cloudApiKey);
  const [baseUrlInput, setBaseUrlInput] = React.useState(cloudBaseUrl);

  React.useEffect(() => {
    setApiKeyInput(cloudApiKey);
  }, [cloudApiKey]);

  React.useEffect(() => {
    setBaseUrlInput(cloudBaseUrl);
  }, [cloudBaseUrl]);

  const backendOptions = React.useMemo<SelectOption[]>(
    () => [
      {
        value: "local",
        label: t("settings.general.transcriptionBackend.options.local"),
      },
      {
        value: "groq_cloud",
        label: t("settings.general.transcriptionBackend.options.groqCloud"),
      },
    ],
    [t],
  );

  const cloudModelOptions = React.useMemo<SelectOption[]>(() => {
    const options = cloudSttModelOptions[cloudProviderId] || [];
    const seen = new Set<string>();
    const merged: SelectOption[] = [];

    const pushOption = (value: string | null | undefined) => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) return;
      seen.add(trimmed);
      merged.push({ value: trimmed, label: trimmed });
    };

    options.forEach(pushOption);
    pushOption(cloudModel);

    return merged;
  }, [cloudSttModelOptions, cloudProviderId, cloudModel]);

  const maxAudioOptions = React.useMemo<SelectOption[]>(() => {
    const values = Array.from(
      new Set([cloudMaxAudioSeconds, ...CLOUD_STT_MAX_AUDIO_SECONDS]),
    ).sort((a, b) => a - b);

    return values.map((seconds) => ({
      value: String(seconds),
      label: t("settings.general.cloudStt.secondsLabel", { seconds }),
    }));
  }, [cloudMaxAudioSeconds, t]);

  const timeoutOptions = React.useMemo<SelectOption[]>(() => {
    const values = Array.from(
      new Set([cloudRequestTimeoutSeconds, ...CLOUD_STT_TIMEOUT_SECONDS]),
    ).sort((a, b) => a - b);

    return values.map((seconds) => ({
      value: String(seconds),
      label: t("settings.general.cloudStt.secondsLabel", { seconds }),
    }));
  }, [cloudRequestTimeoutSeconds, t]);

  const isCloudBackend = transcriptionBackend === "groq_cloud";
  const isCloudModelUpdating = isUpdating(`cloud_stt_model:${cloudProviderId}`);
  const isCloudModelsFetching = isUpdating(
    `cloud_stt_models_fetch:${cloudProviderId}`,
  );

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.general.title")}>
        <ShortcutInput shortcutId="transcribe" grouped={true} />
        <PushToTalk descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>
      <SettingsGroup title={t("settings.general.transcriptionBackend.title")}>
        <SettingContainer
          title={t("settings.general.transcriptionBackend.title")}
          description={t("settings.general.transcriptionBackend.description")}
          grouped={true}
        >
          <Select
            value={transcriptionBackend}
            options={backendOptions}
            onChange={(value) => {
              if (!value) return;
              void updateSetting(
                "transcription_backend",
                value as TranscriptionBackend,
              );
            }}
            disabled={isUpdating("transcription_backend")}
            isClearable={false}
            className="min-w-[240px]"
          />
        </SettingContainer>

        {isCloudBackend && (
          <>
            <SettingContainer
              title={t("settings.general.cloudStt.apiKey.title")}
              description={t("settings.general.cloudStt.apiKey.description")}
              grouped={true}
            >
              <Input
                type="password"
                value={apiKeyInput}
                onChange={(event) => setApiKeyInput(event.target.value)}
                onBlur={() => {
                  const trimmed = apiKeyInput.trim();
                  if (trimmed !== cloudApiKey) {
                    void updateCloudSttApiKey(cloudProviderId, trimmed);
                  }
                }}
                placeholder={t("settings.general.cloudStt.apiKey.placeholder")}
                variant="compact"
                disabled={isUpdating(`cloud_stt_api_key:${cloudProviderId}`)}
                className="min-w-[280px]"
              />
            </SettingContainer>

            <SettingContainer
              title={t("settings.general.cloudStt.model.title")}
              description={t("settings.general.cloudStt.model.description")}
              grouped={true}
            >
              <div className="flex items-center gap-2">
                <Select
                  className="min-w-[280px] flex-1 text-sm"
                  value={cloudModel || null}
                  options={cloudModelOptions}
                  onChange={(value) => {
                    void updateCloudSttModel(cloudProviderId, value ?? "");
                  }}
                  onCreateOption={(value) => {
                    const trimmed = value.trim();
                    if (!trimmed) return;
                    void updateCloudSttModel(cloudProviderId, trimmed);
                  }}
                  onBlur={() => {
                    const trimmed = cloudModel.trim();
                    if (trimmed !== cloudModel) {
                      void updateCloudSttModel(cloudProviderId, trimmed);
                    }
                  }}
                  placeholder={t("settings.general.cloudStt.model.placeholder")}
                  disabled={isCloudModelUpdating}
                  isLoading={isCloudModelsFetching}
                  isCreatable
                  formatCreateLabel={(input) => `Use "${input}"`}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => void fetchCloudSttModels(cloudProviderId)}
                  disabled={
                    isCloudModelsFetching ||
                    isCloudModelUpdating ||
                    !cloudApiKey.trim()
                  }
                >
                  {t("settings.general.cloudStt.model.refreshModels")}
                </Button>
              </div>
            </SettingContainer>

            <SettingContainer
              title={t("settings.general.cloudStt.baseUrl.title")}
              description={t("settings.general.cloudStt.baseUrl.description")}
              grouped={true}
            >
              <Input
                type="text"
                value={baseUrlInput}
                onChange={(event) => setBaseUrlInput(event.target.value)}
                onBlur={() => {
                  const trimmed = baseUrlInput.trim();
                  if (trimmed && trimmed !== cloudBaseUrl) {
                    void updateCloudSttBaseUrl(cloudProviderId, trimmed);
                  }
                }}
                placeholder={t("settings.general.cloudStt.baseUrl.placeholder")}
                variant="compact"
                disabled={isUpdating(`cloud_stt_base_url:${cloudProviderId}`)}
                className="min-w-[280px]"
              />
            </SettingContainer>

            <ToggleSwitch
              checked={cloudFallbackToLocal}
              onChange={(enabled) =>
                void updateSetting("cloud_stt_fallback_to_local", enabled)
              }
              isUpdating={isUpdating("cloud_stt_fallback_to_local")}
              label={t("settings.general.cloudStt.fallbackToLocal.label")}
              description={t(
                "settings.general.cloudStt.fallbackToLocal.description",
              )}
              grouped={true}
            />

            <ToggleSwitch
              checked={cloudPreloadLocalModel}
              onChange={(enabled) =>
                void updateSetting("cloud_stt_preload_local_model", enabled)
              }
              isUpdating={isUpdating("cloud_stt_preload_local_model")}
              label={t("settings.general.cloudStt.preloadLocalModel.label")}
              description={t(
                "settings.general.cloudStt.preloadLocalModel.description",
              )}
              grouped={true}
            />

            <SettingContainer
              title={t("settings.general.cloudStt.maxAudioSeconds.title")}
              description={t(
                "settings.general.cloudStt.maxAudioSeconds.description",
              )}
              grouped={true}
            >
              <Select
                value={String(cloudMaxAudioSeconds)}
                options={maxAudioOptions}
                onChange={(value) => {
                  if (!value) return;
                  const seconds = Number(value);
                  if (!Number.isFinite(seconds)) return;
                  void updateSetting("cloud_stt_max_audio_seconds", seconds);
                }}
                disabled={isUpdating("cloud_stt_max_audio_seconds")}
                isClearable={false}
                className="min-w-[200px]"
              />
            </SettingContainer>

            <SettingContainer
              title={t("settings.general.cloudStt.requestTimeoutSeconds.title")}
              description={t(
                "settings.general.cloudStt.requestTimeoutSeconds.description",
              )}
              grouped={true}
            >
              <Select
                value={String(cloudRequestTimeoutSeconds)}
                options={timeoutOptions}
                onChange={(value) => {
                  if (!value) return;
                  const seconds = Number(value);
                  if (!Number.isFinite(seconds)) return;
                  void updateSetting(
                    "cloud_stt_request_timeout_seconds",
                    seconds,
                  );
                }}
                disabled={isUpdating("cloud_stt_request_timeout_seconds")}
                isClearable={false}
                className="min-w-[200px]"
              />
            </SettingContainer>
          </>
        )}
      </SettingsGroup>
      <ModelSettingsCard />
      <SettingsGroup title={t("settings.sound.title")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <AudioFeedback descriptionMode="tooltip" grouped={true} />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
        />
        <VolumeSlider disabled={!audioFeedbackEnabled} />
      </SettingsGroup>
    </div>
  );
};
