#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <cstring>
#include <memory>

using Microsoft::WRL::ComPtr;

struct Reader {
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    ComPtr<IDXGIOutputDuplication> output;
    ComPtr<ID3D11Texture2D> staging;
    D3D11_BOX crop{};
    UINT width{}, height{};
};

extern "C" __declspec(dllexport) HRESULT open_strip(int x, int y, UINT width, UINT height, Reader** result) {
    wchar_t runner[32]{};
    if (!GetEnvironmentVariableW(L"MOTION_RUNNER", runner, 32) || wcscmp(runner, L"github-hosted") || !result || !width || !height || (width != 1 && height != 1)) return E_ACCESSDENIED;
    *result = nullptr;
    auto reader = std::make_unique<Reader>();
    ComPtr<IDXGIFactory1> factory;
    HRESULT status = CreateDXGIFactory1(IID_PPV_ARGS(factory.GetAddressOf()));
    if (FAILED(status)) return status;
    for (UINT index = 0; ; ++index) {
        ComPtr<IDXGIAdapter1> adapter;
        status = factory->EnumAdapters1(index, &adapter);
        if (FAILED(status)) return status;
        for (UINT display = 0; ; ++display) {
            ComPtr<IDXGIOutput> output;
            status = adapter->EnumOutputs(display, &output);
            if (status == DXGI_ERROR_NOT_FOUND) break;
            if (FAILED(status)) return status;
            DXGI_OUTPUT_DESC desc{};
            if (FAILED(status = output->GetDesc(&desc))) return status;
            const RECT area = desc.DesktopCoordinates;
            if (!desc.AttachedToDesktop || x < area.left || y < area.top || x + static_cast<int>(width) > area.right || y + static_cast<int>(height) > area.bottom) continue;
            if (desc.Rotation != DXGI_MODE_ROTATION_IDENTITY && desc.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED) return E_NOTIMPL;
            status = D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, 0, nullptr, 0, D3D11_SDK_VERSION, &reader->device, nullptr, &reader->context);
            if (FAILED(status)) return status;
            ComPtr<IDXGIOutput1> extended;
            if (FAILED(status = output.As(&extended))) return status;
            if (FAILED(status = extended->DuplicateOutput(reader->device.Get(), &reader->output))) return status;
            D3D11_TEXTURE2D_DESC target{};
            target.Width = width; target.Height = height; target.MipLevels = 1; target.ArraySize = 1;
            target.Format = DXGI_FORMAT_B8G8R8A8_UNORM; target.SampleDesc.Count = 1;
            target.Usage = D3D11_USAGE_STAGING; target.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
            if (FAILED(status = reader->device->CreateTexture2D(&target, nullptr, &reader->staging))) return status;
            reader->width = width; reader->height = height;
            auto left = static_cast<UINT>(x - area.left), top = static_cast<UINT>(y - area.top);
            reader->crop = {left, top, 0, left + width, top + height, 1};
            *result = reader.release();
            return S_OK;
        }
    }
}

extern "C" __declspec(dllexport) HRESULT next_strip(Reader* reader, BYTE* pixels, LONGLONG* presented, UINT* accumulated) {
    DXGI_OUTDUPL_FRAME_INFO info{};
    ComPtr<IDXGIResource> surface;
    HRESULT status = reader->output->AcquireNextFrame(25, &info, &surface);
    if (status == DXGI_ERROR_WAIT_TIMEOUT) return S_FALSE;
    if (FAILED(status)) return status;
    struct Release { IDXGIOutputDuplication* output; ~Release() { output->ReleaseFrame(); } } frame{reader->output.Get()};
    if (!info.LastPresentTime.QuadPart) return S_FALSE;
    ComPtr<ID3D11Texture2D> texture;
    if (FAILED(status = surface.As(&texture))) return status;
    reader->context->CopySubresourceRegion(reader->staging.Get(), 0, 0, 0, 0, texture.Get(), 0, &reader->crop);
    D3D11_MAPPED_SUBRESOURCE mapped{};
    if (FAILED(status = reader->context->Map(reader->staging.Get(), 0, D3D11_MAP_READ, 0, &mapped))) return status;
    for (UINT row = 0; row < reader->height; ++row) std::memcpy(pixels + row * reader->width * 4, static_cast<const BYTE*>(mapped.pData) + row * mapped.RowPitch, reader->width * 4);
    reader->context->Unmap(reader->staging.Get(), 0);
    *presented = info.LastPresentTime.QuadPart; *accumulated = info.AccumulatedFrames;
    return S_OK;
}

extern "C" __declspec(dllexport) void close_strip(Reader* reader) { delete reader; }
