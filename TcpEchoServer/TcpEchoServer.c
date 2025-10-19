#define _WIN32_WINNT 0x0601
#include <winsock2.h>
#include <ws2tcpip.h>
#include <stdio.h>

#pragma comment(lib, "Ws2_32.lib")

static int sendData(SOCKET client_sock, int sentCount)
{
    char buf[1024];
    int dataSize = sprintf_s(buf, sizeof(buf), "Data Count: %d\n", sentCount);
    int sentTotalSize = 0;

    while (sentTotalSize < dataSize) {
        int sentSize = send(client_sock, buf + sentTotalSize, dataSize - sentTotalSize, 0);
        if (sentSize == SOCKET_ERROR) {
            fprintf(stderr, "send failed: %d\n", WSAGetLastError());
            return 1;
        }
        else if (sentSize == 0) {
            printf("Client disconnected\n");
        }
        sentTotalSize += sentSize;
    }

    return 0;
}

static int handle_client(SOCKET client_sock)
{
    int ret = 1;
    char buf[256];
    int sentCount = 0;

    while (1) {
        // ブロッキングで要求を待つ（クライアントが送るまでここで待機）
        int recvlen = recv(client_sock, buf, sizeof(buf) - 1, 0);
        if (recvlen == 0) {
            // クライアントが接続を閉じた
            printf("Client closed connection\n");
            break;
        }
        if (recvlen == SOCKET_ERROR) {
            fprintf(stderr, "recv failed: %d\n", WSAGetLastError());
            break;
        }

        buf[recvlen] = '\0';
        if (strncmp(buf, "GET", 3) == 0) {
            ++sentCount;
            if (sendData(client_sock, sentCount) != 0) {
                fprintf(stderr, "sendData failed\n");
                break;
            }
        }
        else if (strncmp(buf, "QUIT", 4) == 0) {
            // クライアントが終了を要求したら切断
            ret = 0;
            break;
        }
        else {
            // 不明な要求は無視またはエラーメッセージを返す
            const char* resp = "ERR\n";
            send(client_sock, resp, (int)strlen(resp), 0);
        }
    }

    closesocket(client_sock);
    return ret;
}

int main(void) {
    WSADATA wsa;
    SOCKET listen_sock = INVALID_SOCKET;
    SOCKET client_sock = INVALID_SOCKET;
    struct sockaddr_in server_addr;
    int ret;

    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "WSAStartup failed\n");
        return 1;
    }

    listen_sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (listen_sock == INVALID_SOCKET) {
        fprintf(stderr, "socket failed: %d\n", WSAGetLastError());
        WSACleanup();
        return 1;
    }

    ZeroMemory(&server_addr, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_port = htons(4000);
    if (InetPtonA(AF_INET, "127.0.0.1", &server_addr.sin_addr) != 1) {
        fprintf(stderr, "InetPtonA failed\n");
        closesocket(listen_sock);
        WSACleanup();
        return 1;
    }

    ret = bind(listen_sock, (struct sockaddr*)&server_addr, sizeof(server_addr));
    if (ret == SOCKET_ERROR) {
        fprintf(stderr, "bind failed: %d\n", WSAGetLastError());
        closesocket(listen_sock);
        WSACleanup();
        return 1;
    }

    ret = listen(listen_sock, SOMAXCONN);
    if (ret == SOCKET_ERROR) {
        fprintf(stderr, "listen failed: %d\n", WSAGetLastError());
        closesocket(listen_sock);
        WSACleanup();
        return 1;
    }

    printf("Tcp server listening on 127.0.0.1:4000\n");

    while (1) {
        client_sock = accept(listen_sock, NULL, NULL);
        if (client_sock == INVALID_SOCKET) {
            fprintf(stderr, "accept failed: %d\n", WSAGetLastError());
            break;
        }

        printf("Client connected\n");

        handle_client(client_sock);
    }

    closesocket(listen_sock);
    WSACleanup();
    return 0;
}