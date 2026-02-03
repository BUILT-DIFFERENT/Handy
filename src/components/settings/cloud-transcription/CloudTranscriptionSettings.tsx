import React, { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw } from "lucide-react";

import { Alert } from "../../ui/Alert";
import { SettingContainer, SettingsGroup } from "../../ui";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { ResetButton } from "../../ui/ResetButton";
import { Dropdown, type DropdownOption } from "../../ui/Dropdown";

import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { BaseUrlField } from "../PostProcessingSettingsApi/BaseUrlField";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { useCloudTranscriptionProviderState } from "./useCloudTranscriptionProviderState";
import { useSettings } from "../../../hooks/useSettings";
import { useModelStore } from "../../../stores/modelStore";
import { getTranslatedModelName } from "../../../lib/utils/modelTranslation";

const DisabledNotice: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => (
  <div className="p-4 bg-mid-gray/5 rounded-lg border border-mid-gray/20">
    <p className="text-sm text-mid-gray">{children}</p>
  </div>
);

export const CloudTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const state = useCloudTranscriptionProviderState();
  const fallbackEnabled =
    getSetting("cloud_transcription_fallback_enabled") || false;
  const fallbackModelId =
    getSetting("cloud_transcription_fallback_model_id") || "";

  const {
    models,
    currentModel,
    initialize,
    loading: modelsLoading,
  } = useModelStore();

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const availableModels = useMemo(
    () => models.filter((model) => model.is_downloaded),
    [models],
  );

  const fallbackOptions = useMemo<DropdownOption[]>(
    () =>
      availableModels.map((model) => ({
        value: model.id,
        label: getTranslatedModelName(model, t),
      })),
    [availableModels, t],
  );

  return (
    <SettingsGroup title={t("settings.cloudTranscription.groupTitle")}>
      <ToggleSwitch
        checked={state.enabled}
        onChange={(enabled) =>
          updateSetting("cloud_transcription_enabled", enabled)
        }
        isUpdating={isUpdating("cloud_transcription_enabled")}
        label={t("settings.cloudTranscription.toggle.label")}
        description={t("settings.cloudTranscription.toggle.description")}
        descriptionMode="tooltip"
        grouped={true}
      />

      {!state.enabled ? (
        <DisabledNotice>
          {t("settings.cloudTranscription.disabledNotice")}
        </DisabledNotice>
      ) : (
        <>
          <SettingContainer
            title={t("settings.cloudTranscription.provider.title")}
            description={t("settings.cloudTranscription.provider.description")}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ProviderSelect
                options={state.providerOptions}
                value={state.selectedProviderId}
                onChange={state.handleProviderSelect}
              />
            </div>
          </SettingContainer>

          {state.isCustomProvider && (
            <SettingContainer
              title={t("settings.cloudTranscription.baseUrl.title")}
              description={t("settings.cloudTranscription.baseUrl.description")}
              descriptionMode="tooltip"
              layout="horizontal"
              grouped={true}
            >
              <div className="flex items-center gap-2">
                <BaseUrlField
                  value={state.baseUrl}
                  onBlur={state.handleBaseUrlChange}
                  placeholder={t("settings.cloudTranscription.baseUrl.placeholder")}
                  disabled={state.isBaseUrlUpdating}
                  className="min-w-[380px]"
                />
              </div>
            </SettingContainer>
          )}

          <SettingContainer
            title={t("settings.cloudTranscription.apiKey.title")}
            description={t("settings.cloudTranscription.apiKey.description")}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ApiKeyField
                value={state.apiKey}
                onBlur={state.handleApiKeyChange}
                placeholder={t("settings.cloudTranscription.apiKey.placeholder")}
                disabled={state.isApiKeyUpdating}
                className="min-w-[320px]"
              />
            </div>
          </SettingContainer>

          <SettingContainer
            title={t("settings.cloudTranscription.model.title")}
            description={t("settings.cloudTranscription.model.description")}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ModelSelect
                value={state.model}
                options={state.modelOptions}
                disabled={state.isModelUpdating}
                isLoading={state.isFetchingModels}
                placeholder={
                  state.modelOptions.length > 0
                    ? t("settings.cloudTranscription.model.placeholderWithOptions")
                    : t("settings.cloudTranscription.model.placeholderNoOptions")
                }
                onSelect={state.handleModelSelect}
                onCreate={state.handleModelCreate}
                onBlur={() => {}}
                className="flex-1 min-w-[380px]"
              />
              <ResetButton
                onClick={state.handleRefreshModels}
                disabled={state.isFetchingModels}
                ariaLabel={t("settings.cloudTranscription.model.refreshModels")}
                className="flex h-10 w-10 items-center justify-center"
              >
                <RefreshCcw
                  className={`h-4 w-4 ${state.isFetchingModels ? "animate-spin" : ""}`}
                />
              </ResetButton>
            </div>
          </SettingContainer>

          <Alert variant="info" contained>
            {t("settings.cloudTranscription.limitsNotice")}
          </Alert>

          <ToggleSwitch
            checked={fallbackEnabled}
            onChange={(enabled) =>
              updateSetting("cloud_transcription_fallback_enabled", enabled)
            }
            isUpdating={isUpdating("cloud_transcription_fallback_enabled")}
            label={t("settings.cloudTranscription.fallback.label")}
            description={t("settings.cloudTranscription.fallback.description")}
            descriptionMode="tooltip"
            grouped={true}
          />

          <SettingContainer
            title={t("settings.cloudTranscription.fallbackModel.title")}
            description={t("settings.cloudTranscription.fallbackModel.description")}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <Dropdown
                options={fallbackOptions}
                selectedValue={fallbackModelId || currentModel || null}
                onSelect={(value) => {
                  void updateSetting(
                    "cloud_transcription_fallback_model_id",
                    value,
                  );
                }}
                disabled={!fallbackEnabled || modelsLoading}
                placeholder={t("settings.cloudTranscription.fallbackModel.placeholder")}
                className="min-w-[260px]"
              />
            </div>
          </SettingContainer>

          {fallbackOptions.length === 0 && (
            <Alert variant="warning" contained>
              {t("settings.cloudTranscription.fallbackModel.emptyNotice")}
            </Alert>
          )}
        </>
      )}
    </SettingsGroup>
  );
};
