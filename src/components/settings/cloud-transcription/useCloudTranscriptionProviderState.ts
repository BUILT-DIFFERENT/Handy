import { useCallback, useMemo } from "react";
import type { TranscriptionProvider } from "@/bindings";
import type { DropdownOption } from "../../ui/Dropdown";
import type { ModelOption } from "../PostProcessingSettingsApi/types";
import { useSettings } from "../../../hooks/useSettings";

export type CloudTranscriptionProviderState = {
  enabled: boolean;
  providerOptions: DropdownOption[];
  selectedProviderId: string;
  selectedProvider: TranscriptionProvider | undefined;
  isCustomProvider: boolean;
  baseUrl: string;
  apiKey: string;
  model: string;
  modelOptions: ModelOption[];
  isFetchingModels: boolean;
  isBaseUrlUpdating: boolean;
  isApiKeyUpdating: boolean;
  isModelUpdating: boolean;
  handleProviderSelect: (providerId: string) => void;
  handleBaseUrlChange: (value: string) => void;
  handleApiKeyChange: (value: string) => void;
  handleModelSelect: (value: string) => void;
  handleModelCreate: (value: string) => void;
  handleRefreshModels: () => void;
};

export const useCloudTranscriptionProviderState =
  (): CloudTranscriptionProviderState => {
    const {
      settings,
      isUpdating,
      setTranscriptionProvider,
      updateTranscriptionBaseUrl,
      updateTranscriptionApiKey,
      updateTranscriptionModel,
      fetchTranscriptionModels,
      transcriptionModelOptions,
    } = useSettings();

    const providers = settings?.transcription_providers || [];

    const selectedProviderId = useMemo(() => {
      return settings?.transcription_provider_id || providers[0]?.id || "groq";
    }, [providers, settings?.transcription_provider_id]);

    const selectedProvider = useMemo(() => {
      return (
        providers.find((provider) => provider.id === selectedProviderId) ||
        providers[0]
      );
    }, [providers, selectedProviderId]);

    const enabled = settings?.cloud_transcription_enabled ?? false;
    const isCustomProvider = selectedProvider?.id === "custom";
    const baseUrl = selectedProvider?.base_url ?? "";
    const apiKey = settings?.transcription_api_keys?.[selectedProviderId] ?? "";
    const model = settings?.transcription_models?.[selectedProviderId] ?? "";

    const providerOptions = useMemo<DropdownOption[]>(() => {
      return providers.map((provider) => ({
        value: provider.id,
        label: provider.label,
      }));
    }, [providers]);

    const handleProviderSelect = useCallback(
      (providerId: string) => {
        if (providerId === selectedProviderId) return;
        void setTranscriptionProvider(providerId);
      },
      [selectedProviderId, setTranscriptionProvider],
    );

    const handleBaseUrlChange = useCallback(
      (value: string) => {
        if (!selectedProvider || !selectedProvider.allow_base_url_edit) return;
        const trimmed = value.trim();
        if (trimmed === baseUrl) return;
        void updateTranscriptionBaseUrl(selectedProvider.id, trimmed);
      },
      [selectedProvider, baseUrl, updateTranscriptionBaseUrl],
    );

    const handleApiKeyChange = useCallback(
      (value: string) => {
        const trimmed = value.trim();
        if (trimmed === apiKey) return;
        void updateTranscriptionApiKey(selectedProviderId, trimmed);
      },
      [apiKey, selectedProviderId, updateTranscriptionApiKey],
    );

    const handleModelSelect = useCallback(
      (value: string) => {
        void updateTranscriptionModel(selectedProviderId, value.trim());
      },
      [selectedProviderId, updateTranscriptionModel],
    );

    const handleModelCreate = useCallback(
      (value: string) => {
        void updateTranscriptionModel(selectedProviderId, value);
      },
      [selectedProviderId, updateTranscriptionModel],
    );

    const handleRefreshModels = useCallback(() => {
      void fetchTranscriptionModels(selectedProviderId);
    }, [fetchTranscriptionModels, selectedProviderId]);

    const availableModelsRaw = transcriptionModelOptions[selectedProviderId] || [];
    const modelOptions = useMemo<ModelOption[]>(() => {
      return availableModelsRaw.map((value) => ({ value, label: value }));
    }, [availableModelsRaw]);

    const isBaseUrlUpdating = isUpdating(
      `transcription_base_url:${selectedProviderId}`,
    );
    const isApiKeyUpdating = isUpdating(
      `transcription_api_key:${selectedProviderId}`,
    );
    const isModelUpdating = isUpdating(
      `transcription_model:${selectedProviderId}`,
    );
    const isFetchingModels = isUpdating(
      `transcription_models_fetch:${selectedProviderId}`,
    );

    return {
      enabled,
      providerOptions,
      selectedProviderId,
      selectedProvider,
      isCustomProvider,
      baseUrl,
      apiKey,
      model,
      modelOptions,
      isFetchingModels,
      isBaseUrlUpdating,
      isApiKeyUpdating,
      isModelUpdating,
      handleProviderSelect,
      handleBaseUrlChange,
      handleApiKeyChange,
      handleModelSelect,
      handleModelCreate,
      handleRefreshModels,
    };
  };
