package com.mengying.fqnovel.sidecar;

import com.mengying.fqnovel.dto.ApiEnvelope;
import com.mengying.fqnovel.service.RegisterKeyVersionMismatchException;
import com.mengying.fqnovel.service.SignerUnavailableException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(IllegalArgumentException.class)
    public ResponseEntity<ApiEnvelope<Void>> handleBadRequest(IllegalArgumentException exception) {
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
            .body(ApiEnvelope.error(1001, exception.getMessage()));
    }

    @ExceptionHandler(SignerUnavailableException.class)
    public ResponseEntity<ApiEnvelope<Void>> handleSignerUnavailable(SignerUnavailableException exception) {
        return ResponseEntity.status(HttpStatus.SERVICE_UNAVAILABLE)
            .body(ApiEnvelope.error(1003, exception.getMessage()));
    }

    @ExceptionHandler(RegisterKeyVersionMismatchException.class)
    public ResponseEntity<ApiEnvelope<Void>> handleRegisterKeyMismatch(RegisterKeyVersionMismatchException exception) {
        return ResponseEntity.status(HttpStatus.CONFLICT)
            .body(ApiEnvelope.error(1101, exception.getMessage()));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<ApiEnvelope<Void>> handleInternalError(Exception exception) {
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
            .body(ApiEnvelope.error(1500, "internal sidecar error"));
    }
}

